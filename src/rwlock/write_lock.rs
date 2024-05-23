use std::fmt::Debug;

use lock_api::RawRwLock;

use crate::key::Keyable;

use super::{RwLock, RwLockWriteGuard, WriteLock};

impl<'l, T: ?Sized + Debug, R: RawRwLock> Debug for WriteLock<'l, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		// safety: this is just a try lock, and the value is dropped
		//         immediately after, so there's no risk of blocking ourselves
		//         or any other threads
		// It makes zero sense to try using an exclusive lock for this, so this
		// is the only time when WriteLock does a read.
		if let Some(value) = unsafe { self.0.try_read_no_key() } {
			f.debug_struct("WriteLock").field("data", &&*value).finish()
		} else {
			struct LockedPlaceholder;
			impl Debug for LockedPlaceholder {
				fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
					f.write_str("<locked>")
				}
			}

			f.debug_struct("WriteLock")
				.field("data", &LockedPlaceholder)
				.finish()
		}
	}
}

impl<'l, T, R> From<&'l RwLock<T, R>> for WriteLock<'l, T, R> {
	fn from(value: &'l RwLock<T, R>) -> Self {
		Self::new(value)
	}
}

impl<'l, T: ?Sized, R> AsRef<RwLock<T, R>> for WriteLock<'l, T, R> {
	fn as_ref(&self) -> &RwLock<T, R> {
		self.0
	}
}

impl<'l, T, R> WriteLock<'l, T, R> {
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
	pub const fn new(rwlock: &'l RwLock<T, R>) -> Self {
		Self(rwlock)
	}
}

impl<'l, T: ?Sized, R: RawRwLock> WriteLock<'l, T, R> {
	/// Locks the underlying [`RwLock`] with exclusive write access, blocking
	/// the current until it can be acquired.
	pub fn lock<'s, 'key: 's, Key: Keyable + 'key>(
		&'s self,
		key: Key,
	) -> RwLockWriteGuard<'_, 'key, T, Key, R> {
		self.0.write(key)
	}

	/// Attempts to lock the underlying [`RwLock`] with exclusive write access.
	pub fn try_lock<'s, 'key: 's, Key: Keyable + 'key>(
		&'s self,
		key: Key,
	) -> Option<RwLockWriteGuard<'_, 'key, T, Key, R>> {
		self.0.try_write(key)
	}

	// There's no `try_lock_no_key`. Instead, `try_read_no_key` is called on
	// the referenced `RwLock`.

	/// Immediately drops the guard, and consequently releases the exclusive
	/// lock.
	pub fn unlock<'key, Key: Keyable + 'key>(guard: RwLockWriteGuard<'_, 'key, T, Key, R>) -> Key {
		RwLock::unlock_write(guard)
	}
}
