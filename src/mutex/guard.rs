use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use lock_api::RawMutex;

use crate::key::Keyable;

use super::{Mutex, MutexGuard, MutexRef};

impl<'a, T: ?Sized + 'a, R: RawMutex> Drop for MutexRef<'a, T, R> {
	fn drop(&mut self) {
		// safety: this guard is being destroyed, so the data cannot be
		//         accessed without locking again
		unsafe { self.0.force_unlock() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawMutex> Deref for MutexRef<'a, T, R> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// safety: this is the only type that can use `value`, and there's
		//         a reference to this type, so there cannot be any mutable
		//         references to this value.
		unsafe { &*self.0.data.get() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawMutex> DerefMut for MutexRef<'a, T, R> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		// safety: this is the only type that can use `value`, and we have a
		//         mutable reference to this type, so there cannot be any other
		//         references to this value.
		unsafe { &mut *self.0.data.get() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawMutex> MutexRef<'a, T, R> {
	pub unsafe fn new(mutex: &'a Mutex<T, R>) -> Self {
		Self(mutex, PhantomData)
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawMutex> Deref
	for MutexGuard<'a, 'key, T, Key, R>
{
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.mutex
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawMutex> DerefMut
	for MutexGuard<'a, 'key, T, Key, R>
{
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.mutex
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawMutex> MutexGuard<'a, 'key, T, Key, R> {
	/// Create a guard to the given mutex. Undefined if multiple guards to the
	/// same mutex exist at once.
	#[must_use]
	pub(super) unsafe fn new(mutex: &'a Mutex<T, R>, thread_key: Key) -> Self {
		Self {
			mutex: MutexRef(mutex, PhantomData),
			thread_key,
			_phantom: PhantomData,
		}
	}
}

unsafe impl<'a, T: ?Sized + Sync + 'a, R: RawMutex + Sync + 'a> Sync for MutexRef<'a, T, R> {}
