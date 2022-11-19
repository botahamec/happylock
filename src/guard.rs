use std::ops::{Deref, DerefMut};

use crate::{
	mutex::{MutexRef, RawMutex},
	Mutex, ThreadKey,
};

mod sealed {
	#[allow(clippy::wildcard_imports)]
	use super::*;
	pub trait Sealed {}
	impl<R: RawMutex, T> Sealed for Mutex<R, T> {}
	impl<'a, A: Lockable<'a>, B: Lockable<'a>> Sealed for (A, B) {}
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

impl<'a, R: RawMutex + 'a, T: 'a> Lockable<'a> for Mutex<R, T> {
	type Output = MutexRef<'a, R, T>;

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
