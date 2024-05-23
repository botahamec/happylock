use std::fmt::{Debug, Display};
use std::marker::PhantomData;
use std::ops::Deref;

use lock_api::RawRwLock;

use crate::key::Keyable;

use super::{RwLock, RwLockReadGuard, RwLockReadRef};

impl<'a, T: Debug + ?Sized + 'a, R: RawRwLock> Debug for RwLockReadRef<'a, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<'a, T: Display + ?Sized + 'a, R: RawRwLock> Display for RwLockReadRef<'a, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}

impl<'a, T: ?Sized + 'a, R: RawRwLock> Deref for RwLockReadRef<'a, T, R> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// safety: this is the only type that can use `value`, and there's
		//         a reference to this type, so there cannot be any mutable
		//         references to this value.
		unsafe { &*self.0.data.get() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawRwLock> AsRef<T> for RwLockReadRef<'a, T, R> {
	fn as_ref(&self) -> &T {
		self
	}
}

impl<'a, T: ?Sized + 'a, R: RawRwLock> Drop for RwLockReadRef<'a, T, R> {
	fn drop(&mut self) {
		// safety: this guard is being destroyed, so the data cannot be
		//         accessed without locking again
		unsafe { self.0.force_unlock_read() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawRwLock> RwLockReadRef<'a, T, R> {
	/// Creates an immutable reference for the underlying data of an [`RwLock`]
	/// without locking it or taking ownership of the key.
	#[must_use]
	pub(crate) unsafe fn new(mutex: &'a RwLock<T, R>) -> Self {
		Self(mutex, PhantomData)
	}
}

impl<'a, 'key, T: Debug + ?Sized + 'a, Key: Keyable + 'key, R: RawRwLock> Debug
	for RwLockReadGuard<'a, 'key, T, Key, R>
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<'a, 'key, T: Display + ?Sized + 'a, Key: Keyable + 'key, R: RawRwLock> Display
	for RwLockReadGuard<'a, 'key, T, Key, R>
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawRwLock> Deref
	for RwLockReadGuard<'a, 'key, T, Key, R>
{
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.rwlock
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawRwLock> AsRef<T>
	for RwLockReadGuard<'a, 'key, T, Key, R>
{
	fn as_ref(&self) -> &T {
		self
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawRwLock>
	RwLockReadGuard<'a, 'key, T, Key, R>
{
	/// Create a guard to the given mutex. Undefined if multiple guards to the
	/// same mutex exist at once.
	#[must_use]
	pub(super) unsafe fn new(rwlock: &'a RwLock<T, R>, thread_key: Key) -> Self {
		Self {
			rwlock: RwLockReadRef(rwlock, PhantomData),
			thread_key,
			_phantom: PhantomData,
		}
	}
}

unsafe impl<'a, T: ?Sized + Sync + 'a, R: RawRwLock + Sync + 'a> Sync for RwLockReadRef<'a, T, R> {}
