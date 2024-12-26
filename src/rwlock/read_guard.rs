use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::Deref;

use lock_api::RawRwLock;

use crate::key::Keyable;
use crate::lockable::RawLock;

use super::{RwLock, RwLockReadGuard, RwLockReadRef};

impl<T: PartialEq + ?Sized, R: RawRwLock> PartialEq for RwLockReadRef<'_, T, R> {
	fn eq(&self, other: &Self) -> bool {
		self.deref().eq(&**other)
	}
}

impl<T: Eq + ?Sized, R: RawRwLock> Eq for RwLockReadRef<'_, T, R> {}

impl<T: PartialOrd + ?Sized, R: RawRwLock> PartialOrd for RwLockReadRef<'_, T, R> {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		self.deref().partial_cmp(&**other)
	}
}

impl<T: Ord + ?Sized, R: RawRwLock> Ord for RwLockReadRef<'_, T, R> {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.deref().cmp(&**other)
	}
}

impl<T: Hash + ?Sized, R: RawRwLock> Hash for RwLockReadRef<'_, T, R> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.deref().hash(state)
	}
}

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
		unsafe { self.0.raw_unlock_read() }
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

impl<T: PartialEq + ?Sized, R: RawRwLock, Key: Keyable> PartialEq
	for RwLockReadGuard<'_, '_, T, Key, R>
{
	fn eq(&self, other: &Self) -> bool {
		self.deref().eq(&**other)
	}
}

impl<T: Eq + ?Sized, R: RawRwLock, Key: Keyable> Eq for RwLockReadGuard<'_, '_, T, Key, R> {}

impl<T: PartialOrd + ?Sized, R: RawRwLock, Key: Keyable> PartialOrd
	for RwLockReadGuard<'_, '_, T, Key, R>
{
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		self.deref().partial_cmp(&**other)
	}
}

impl<T: Ord + ?Sized, R: RawRwLock, Key: Keyable> Ord for RwLockReadGuard<'_, '_, T, Key, R> {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.deref().cmp(&**other)
	}
}

impl<T: Hash + ?Sized, R: RawRwLock, Key: Keyable> Hash for RwLockReadGuard<'_, '_, T, Key, R> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.deref().hash(state)
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
