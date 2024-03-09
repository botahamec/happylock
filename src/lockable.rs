use std::mem::MaybeUninit;

use crate::mutex::{Mutex, MutexRef};
use lock_api::RawMutex;

mod sealed {
	#[allow(clippy::wildcard_imports)]
	use super::*;

	pub trait Sealed {}
	impl<'a, T, R: RawMutex + 'a> Sealed for Mutex<T, R> {}
	impl<T: Sealed> Sealed for &T {}
	impl<T: Sealed> Sealed for &mut T {}
	impl<'a, A: Lockable<'a>, B: Lockable<'a>> Sealed for (A, B) {}
	impl<'a, T: Lockable<'a>, const N: usize> Sealed for [T; N] {}
	impl<'a, T: Lockable<'a>> Sealed for Vec<T> {}
}

/// A type that may be locked and unlocked
///
/// # Safety
///
/// A deadlock must never occur. The `unlock` method must correctly unlock the
/// data.
pub unsafe trait Lockable<'a>: sealed::Sealed {
	/// The output of the lock
	type Output;

	/// Blocks until the lock is acquired
	///
	/// # Safety
	///
	/// It is undefined behavior to:
	/// * Use this without ownership or mutable access to the [`ThreadKey`],
	/// which should last as long as the return value is alive.
	/// * Call this on multiple locks without unlocking first.
	///
	/// [`ThreadKey`]: `crate::key::ThreadKey`
	unsafe fn lock(&'a self) -> Self::Output;

	/// Attempt to lock without blocking.
	///
	/// Returns `Some` if successful, `None` otherwise.
	///
	/// # Safety
	///
	/// It is undefined behavior to use this without ownership or mutable
	/// access to the [`ThreadKey`], which should last as long as the return
	/// value is alive.
	///
	/// [`ThreadKey`]: `crate::key::ThreadKey`
	unsafe fn try_lock(&'a self) -> Option<Self::Output>;

	/// Release the lock
	fn unlock(guard: Self::Output);
}

unsafe impl<'a, T: Lockable<'a>> Lockable<'a> for &T {
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

unsafe impl<'a, T: Lockable<'a>> Lockable<'a> for &mut T {
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

unsafe impl<'a, T: 'a, R: RawMutex + 'a> Lockable<'a> for Mutex<T, R> {
	type Output = MutexRef<'a, T, R>;

	unsafe fn lock(&'a self) -> Self::Output {
		self.lock_no_key()
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		self.try_lock_no_key()
	}

	fn unlock(guard: Self::Output) {
		drop(guard);
	}
}

unsafe impl<'a, A: Lockable<'a>, B: Lockable<'a>> Lockable<'a> for (A, B) {
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

unsafe impl<'a, T: Lockable<'a>, const N: usize> Lockable<'a> for [T; N] {
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

unsafe impl<'a, T: Lockable<'a>> Lockable<'a> for Vec<T> {
	type Output = Vec<T::Output>;

	unsafe fn lock(&'a self) -> Self::Output {
		loop {
			if let Some(guard) = self.try_lock() {
				return guard;
			}
		}
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		let mut outputs = Vec::new();
		for lock in self {
			if let Some(guard) = lock.try_lock() {
				outputs.push(guard);
			} else {
				Self::unlock(outputs);
				return None;
			};
		}

		Some(outputs)
	}

	fn unlock(guard: Self::Output) {
		for guard in guard {
			T::unlock(guard);
		}
	}
}
