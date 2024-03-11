use std::fmt::Debug;

use lock_api::RawRwLock;

use crate::key::Keyable;

use super::{RwLock, RwLockWriteGuard, RwLockWriteRef, WriteLock};

impl<'a, T: ?Sized, R> Debug for WriteLock<'a, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&format!("WriteLock<{}>", std::any::type_name::<T>()))
	}
}

impl<'a, T: ?Sized, R> From<&'a RwLock<T, R>> for WriteLock<'a, T, R> {
	fn from(value: &'a RwLock<T, R>) -> Self {
		Self::new(value)
	}
}

impl<'a, T: ?Sized, R> AsRef<RwLock<T, R>> for WriteLock<'a, T, R> {
	fn as_ref(&self) -> &RwLock<T, R> {
		self.0
	}
}

impl<'a, T: ?Sized, R> WriteLock<'a, T, R> {
	/// Creates a new `WriteLock` which accesses the given [`RwLock`]
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{rwlock::WriteLock, RwLock};
	///
	/// let lock = RwLock::new(5);
	/// let write_lock = WriteLock::new(&lock);
	/// ```
	#[must_use]
	pub const fn new(rwlock: &'a RwLock<T, R>) -> Self {
		Self(rwlock)
	}
}

impl<'a, T: ?Sized, R: RawRwLock> WriteLock<'a, T, R> {
	/// Locks the underlying [`RwLock`] with exclusive write access, blocking
	/// the current until it can be acquired.
	pub fn lock<'s, 'key: 's, Key: Keyable + 'key>(
		&'s self,
		key: Key,
	) -> RwLockWriteGuard<'_, 'key, T, Key, R> {
		self.0.write(key)
	}

	/// Creates an exclusive lock without a key. Locking this without exclusive
	/// access to the key is undefined behavior.
	pub(crate) unsafe fn lock_no_key(&self) -> RwLockWriteRef<'_, T, R> {
		self.0.write_no_key()
	}

	/// Attempts to lock the underlying [`RwLock`] with exclusive write access.
	pub fn try_lock<'s, 'key: 's, Key: Keyable + 'key>(
		&'s self,
		key: Key,
	) -> Option<RwLockWriteGuard<'_, 'key, T, Key, R>> {
		self.0.try_write(key)
	}

	/// Attempts to create an exclusive lock without a key. Locking this
	/// without exclusive access to the key is undefined behavior.
	pub(crate) unsafe fn try_lock_no_key(&self) -> Option<RwLockWriteRef<'_, T, R>> {
		self.0.try_write_no_key()
	}

	/// Immediately drops the guard, and consequently releases the exclusive
	/// lock.
	pub fn unlock<'key, Key: Keyable + 'key>(guard: RwLockWriteGuard<'_, 'key, T, Key, R>) -> Key {
		RwLock::unlock_write(guard)
	}
}
