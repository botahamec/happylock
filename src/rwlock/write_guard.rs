use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use lock_api::RawRwLock;

use crate::key::Keyable;

use super::{RwLock, RwLockWriteGuard, RwLockWriteRef};

impl<'a, T: ?Sized + 'a, R: RawRwLock> Deref for RwLockWriteRef<'a, T, R> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// safety: this is the only type that can use `value`, and there's
		//         a reference to this type, so there cannot be any mutable
		//         references to this value.
		unsafe { &*self.0.data.get() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawRwLock> DerefMut for RwLockWriteRef<'a, T, R> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		// safety: this is the only type that can use `value`, and we have a
		//         mutable reference to this type, so there cannot be any other
		//         references to this value.
		unsafe { &mut *self.0.data.get() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawRwLock> Drop for RwLockWriteRef<'a, T, R> {
	fn drop(&mut self) {
		// safety: this guard is being destroyed, so the data cannot be
		//         accessed without locking again
		unsafe { self.0.force_unlock_write() }
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawRwLock> Deref
	for RwLockWriteGuard<'a, 'key, T, Key, R>
{
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.rwlock
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawRwLock> DerefMut
	for RwLockWriteGuard<'a, 'key, T, Key, R>
{
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.rwlock
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawRwLock>
	RwLockWriteGuard<'a, 'key, T, Key, R>
{
	/// Create a guard to the given mutex. Undefined if multiple guards to the
	/// same mutex exist at once.
	#[must_use]
	pub(super) const unsafe fn new(rwlock: &'a RwLock<T, R>, thread_key: Key) -> Self {
		Self {
			rwlock: RwLockWriteRef(rwlock),
			thread_key,
			_phantom: PhantomData,
		}
	}
}
