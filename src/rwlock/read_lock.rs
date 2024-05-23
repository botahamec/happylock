use std::fmt::Debug;

use lock_api::RawRwLock;

use crate::key::Keyable;

use super::{ReadLock, RwLock, RwLockReadGuard, RwLockReadRef};

impl<'l, T: ?Sized + Debug, R: RawRwLock> Debug for ReadLock<'l, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		// safety: this is just a try lock, and the value is dropped
		//         immediately after, so there's no risk of blocking ourselves
		//         or any other threads
		if let Some(value) = unsafe { self.try_lock_no_key() } {
			f.debug_struct("ReadLock").field("data", &&*value).finish()
		} else {
			struct LockedPlaceholder;
			impl Debug for LockedPlaceholder {
				fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
					f.write_str("<locked>")
				}
			}

			f.debug_struct("ReadLock")
				.field("data", &LockedPlaceholder)
				.finish()
		}
	}
}

impl<'l, T, R> From<&'l RwLock<T, R>> for ReadLock<'l, T, R> {
	fn from(value: &'l RwLock<T, R>) -> Self {
		Self::new(value)
	}
}

impl<'l, T: ?Sized, R> AsRef<RwLock<T, R>> for ReadLock<'l, T, R> {
	fn as_ref(&self) -> &RwLock<T, R> {
		self.0
	}
}

impl<'l, T, R> ReadLock<'l, T, R> {
	/// Creates a new `ReadLock` which accesses the given [`RwLock`]
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{rwlock::ReadLock, RwLock};
	///
	/// let lock = RwLock::new(5);
	/// let read_lock = ReadLock::new(&lock);
	/// ```
	#[must_use]
	pub const fn new(rwlock: &'l RwLock<T, R>) -> Self {
		Self(rwlock)
	}
}

impl<'l, T: ?Sized, R: RawRwLock> ReadLock<'l, T, R> {
	/// Locks the underlying [`RwLock`] with shared read access, blocking the
	/// current thread until it can be acquired.
	pub fn lock<'s, 'key: 's, Key: Keyable + 'key>(
		&'s self,
		key: Key,
	) -> RwLockReadGuard<'_, 'key, T, Key, R> {
		self.0.read(key)
	}

	/// Attempts to acquire the underlying [`RwLock`] with shared read access
	/// without blocking.
	pub fn try_lock<'s, 'key: 's, Key: Keyable + 'key>(
		&'s self,
		key: Key,
	) -> Option<RwLockReadGuard<'_, 'key, T, Key, R>> {
		self.0.try_read(key)
	}

	/// Attempts to create an exclusive lock without a key. Locking this
	/// without exclusive access to the key is undefined behavior.
	pub(crate) unsafe fn try_lock_no_key(&self) -> Option<RwLockReadRef<'_, T, R>> {
		self.0.try_read_no_key()
	}

	/// Immediately drops the guard, and consequently releases the shared lock
	/// on the underlying [`RwLock`].
	pub fn unlock<'key, Key: Keyable + 'key>(guard: RwLockReadGuard<'_, 'key, T, Key, R>) -> Key {
		RwLock::unlock_read(guard)
	}
}
