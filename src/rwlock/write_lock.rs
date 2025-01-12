use std::fmt::Debug;

use lock_api::RawRwLock;

use crate::key::Keyable;
use crate::lockable::{Lockable, RawLock};

use super::{RwLock, RwLockWriteGuard, RwLockWriteRef, WriteLock};

unsafe impl<T: Send, R: RawRwLock + Send + Sync> Lockable for WriteLock<'_, T, R> {
	type Guard<'g>
		= RwLockWriteRef<'g, T, R>
	where
		Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		ptrs.push(self.as_ref());
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		RwLockWriteRef::new(self.as_ref())
	}
}

// Technically, the exclusive locks can also be shared, but there's currently
// no way to express that. I don't think I want to ever express that.

#[mutants::skip]
impl<T: ?Sized + Debug, R: RawRwLock> Debug for WriteLock<'_, T, R> {
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

impl<T: ?Sized, R> AsRef<RwLock<T, R>> for WriteLock<'_, T, R> {
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

impl<T: ?Sized, R: RawRwLock> WriteLock<'_, T, R> {
	/// Locks the underlying [`RwLock`] with exclusive write access, blocking
	/// the current until it can be acquired.
	///
	/// This function will not return while other writers or readers currently
	/// have access to the lock.
	///
	/// Returns an RAII guard which will drop the write access of this `RwLock`
	/// when dropped.
	///
	/// Because this method takes a [`ThreadKey`], it's not possible for this
	/// method to cause a deadlock.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{ThreadKey, RwLock};
	/// use happylock::rwlock::WriteLock;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = RwLock::new(1);
	/// let writer = WriteLock::new(&lock);
	///
	/// let mut n = writer.lock(key);
	/// *n += 2;
	/// ```
	///
	/// [`ThreadKey`]: `crate::ThreadKey`
	pub fn lock<'s, 'key: 's, Key: Keyable + 'key>(
		&'s self,
		key: Key,
	) -> RwLockWriteGuard<'s, 'key, T, Key, R> {
		self.0.write(key)
	}

	/// Attempts to lock the underlying [`RwLock`] with exclusive write access.
	///
	/// This function does not block. If the lock could not be acquired at this
	/// time, then `None` is returned. Otherwise, an RAII guard is returned
	/// which will release the lock when it is dropped.
	///
	/// This function does not provide any guarantees with respect to the
	/// ordering of whether contentious readers or writers will acquire the
	/// lock first.
	///
	/// # Errors
	///
	/// If the [`RwLock`] could not be acquired because it was already locked,
	/// then an error will be returned containing the given key.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey};
	/// use happylock::rwlock::WriteLock;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = RwLock::new(1);
	/// let writer = WriteLock::new(&lock);
	///
	/// match writer.try_lock(key) {
	///     Ok(n) => assert_eq!(*n, 1),
	///     Err(_) => unreachable!(),
	/// };
	/// ```
	pub fn try_lock<'s, 'key: 's, Key: Keyable + 'key>(
		&'s self,
		key: Key,
	) -> Result<RwLockWriteGuard<'s, 'key, T, Key, R>, Key> {
		self.0.try_write(key)
	}

	// There's no `try_lock_no_key`. Instead, `try_read_no_key` is called on
	// the referenced `RwLock`.

	/// Immediately drops the guard, and consequently releases the exclusive
	/// lock on the underlying [`RwLock`].
	///
	/// This function is equivalent to calling [`drop`] on the guard, except
	/// that it returns the key that was used to create it. Alternately, the
	/// guard will be automatically dropped when it goes out of scope.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey};
	/// use happylock::rwlock::WriteLock;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = RwLock::new(0);
	/// let writer = WriteLock::new(&lock);
	///
	/// let mut guard = writer.lock(key);
	/// *guard += 20;
	/// let key = WriteLock::unlock(guard);
	/// ```
	pub fn unlock<'key, Key: Keyable + 'key>(guard: RwLockWriteGuard<'_, 'key, T, Key, R>) -> Key {
		RwLock::unlock_write(guard)
	}
}
