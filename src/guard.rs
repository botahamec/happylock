use std::{
	mem::MaybeUninit,
	ops::{Deref, DerefMut},
};

use crate::{
	mutex::{Mutex, MutexRef, RawMutex},
	ThreadKey,
};

mod sealed {
	#[allow(clippy::wildcard_imports)]
	use super::*;
	pub trait Sealed {}
	impl<'a, T, R: RawMutex + 'a> Sealed for Mutex<T, R> {}
	impl<T: Sealed> Sealed for &T {}
	impl<T: Sealed> Sealed for &mut T {}
	impl<'a, A: Lockable<'a>, B: Lockable<'a>> Sealed for (A, B) {}
	impl<'a, T: Lockable<'a>, const N: usize> Sealed for [T; N] {}
	impl<'a, T: Lockable<'a>> Sealed for &[T] {}
}

pub trait Lockable<'a>: sealed::Sealed {
	/// The output of the lock
	type Output;

	/// Blocks until the lock is acquired
	///
	/// # Safety
	///
	/// It is undefined behavior to:
	/// * Use this without ownership of the [`ThreadKey`], which should last as
	/// long as the return value is alive.
	/// * Call this on multiple locks without unlocking first.
	unsafe fn lock(&'a self) -> Self::Output;

	/// Attempt to lock without blocking.
	///
	/// Returns `Ok` if successful, `None` otherwise.
	///
	/// # Safety
	///
	/// It is undefined behavior to use this without ownership of the
	/// [`ThreadKey`], which should last as long as the return value is alive.
	unsafe fn try_lock(&'a self) -> Option<Self::Output>;

	/// Release the lock
	fn unlock(guard: Self::Output);
}

impl<'a, T: Lockable<'a>> Lockable<'a> for &T {
	type Output = T::Output;

	unsafe fn lock(&'a self) -> Self::Output {
		(*self).lock()
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		(*self).try_lock()
	}

	#[allow(clippy::semicolon_if_nothing_returned)]
	fn unlock(guard: Self::Output) {
		T::unlock(guard)
	}
}

impl<'a, T: Lockable<'a>> Lockable<'a> for &mut T {
	type Output = T::Output;

	unsafe fn lock(&'a self) -> Self::Output {
		(**self).lock()
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		(**self).try_lock()
	}

	#[allow(clippy::semicolon_if_nothing_returned)]
	fn unlock(guard: Self::Output) {
		T::unlock(guard)
	}
}

impl<'a, T: 'a, R: RawMutex + 'a> Lockable<'a> for Mutex<T, R> {
	type Output = MutexRef<'a, T, R>;

	unsafe fn lock(&'a self) -> Self::Output {
		self.lock_ref()
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		self.try_lock_ref()
	}

	fn unlock(guard: Self::Output) {
		drop(guard);
	}
}

impl<'a, A: Lockable<'a>, B: Lockable<'a>> Lockable<'a> for (A, B) {
	type Output = (A::Output, B::Output);

	unsafe fn lock(&'a self) -> Self::Output {
		loop {
			let lock1 = self.0.lock();
			match self.1.try_lock() {
				Some(lock2) => return (lock1, lock2),
				None => A::unlock(lock1),
			}
		}
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		self.0.try_lock().and_then(|guard1| {
			if let Some(guard2) = self.1.try_lock() {
				Some((guard1, guard2))
			} else {
				A::unlock(guard1);
				None
			}
		})
	}

	fn unlock(guard: Self::Output) {
		A::unlock(guard.0);
		B::unlock(guard.1);
	}
}

impl<'a, T: Lockable<'a>, const N: usize> Lockable<'a> for [T; N] {
	type Output = [T::Output; N];

	unsafe fn lock(&'a self) -> Self::Output {
		loop {
			if let Some(guard) = self.try_lock() {
				return guard;
			}
		}
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		unsafe fn unlock_partial<'a, T: Lockable<'a>, const N: usize>(
			guards: [MaybeUninit<T::Output>; N],
			upto: usize,
		) {
			for (i, guard) in guards.into_iter().enumerate() {
				if i == upto {
					break;
				}
				T::unlock(guard.assume_init());
			}
		}

		let mut outputs = MaybeUninit::<[MaybeUninit<T::Output>; N]>::uninit().assume_init();
		for i in 0..N {
			if let Some(guard) = self[i].try_lock() {
				outputs[i].write(guard)
			} else {
				unlock_partial::<T, N>(outputs, i);
				return None;
			};
		}

		Some(outputs.map(|mu| mu.assume_init()))
	}

	fn unlock(guard: Self::Output) {
		guard.map(T::unlock);
	}
}

pub struct LockGuard<'a, L: Lockable<'a>> {
	guard: L::Output,
	key: ThreadKey,
}

// TODO: docs
impl<'a, L: Lockable<'a>> LockGuard<'a, L> {
	pub fn lock(lock: &'a L, key: ThreadKey) -> Self {
		Self {
			// safety: we have the thread's key
			guard: unsafe { lock.lock() },
			key,
		}
	}

	pub fn try_lock(lock: &'a L, key: ThreadKey) -> Option<Self> {
		// safety: we have the thread's key
		unsafe { lock.try_lock() }.map(|guard| Self { guard, key })
	}

	#[allow(clippy::missing_const_for_fn)]
	pub fn unlock(self) -> ThreadKey {
		L::unlock(self.guard);
		self.key
	}
}

impl<'a, L: Lockable<'a>> Deref for LockGuard<'a, L> {
	type Target = L::Output;

	fn deref(&self) -> &Self::Target {
		&self.guard
	}
}

impl<'a, L: Lockable<'a>> DerefMut for LockGuard<'a, L> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.guard
	}
}
