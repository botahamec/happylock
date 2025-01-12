use std::fmt::Debug;

use lock_api::RawRwLock;

use crate::key::Keyable;
use crate::lockable::{Lockable, RawLock, Sharable};

use super::{ReadLock, RwLock, RwLockReadGuard, RwLockReadRef};

unsafe impl<T: Send, R: RawRwLock + Send + Sync> Lockable for ReadLock<'_, T, R> {
	type Guard<'g>
		= RwLockReadRef<'g, T, R>
	where
		Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		ptrs.push(self.as_ref());
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		RwLockReadRef::new(self.as_ref())
	}
}

unsafe impl<T: Send, R: RawRwLock + Send + Sync> Sharable for ReadLock<'_, T, R> {
	type ReadGuard<'g>
		= RwLockReadRef<'g, T, R>
	where
		Self: 'g;

	unsafe fn read_guard(&self) -> Self::Guard<'_> {
		RwLockReadRef::new(self.as_ref())
	}
}

#[mutants::skip]
impl<T: ?Sized + Debug, R: RawRwLock> Debug for ReadLock<'_, T, R> {
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

impl<T: ?Sized, R> AsRef<RwLock<T, R>> for ReadLock<'_, T, R> {
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

impl<T: ?Sized, R: RawRwLock> ReadLock<'_, T, R> {
	/// Locks the underlying [`RwLock`] with shared read access, blocking the
	/// current thread until it can be acquired.
	///
	/// The calling thread will be blocked until there are no more writers
	/// which hold the lock. There may be other readers currently inside the
	/// lock when this method returns.
	///
	/// Returns an RAII guard which will release this thread's shared access
	/// once it is dropped.
	///
	/// Because this method takes a [`ThreadKey`], it's not possible for this
	/// method to cause a deadlock.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use std::thread;
	/// use happylock::{RwLock, ThreadKey};
	/// use happylock::rwlock::ReadLock;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock: &'static mut RwLock<_> = Box::leak(Box::new(RwLock::new(1)));
	/// let reader = ReadLock::new(&lock);
	///
	/// let n = reader.lock(key);
	/// assert_eq!(*n, 1);
	/// ```
	///
	/// [`ThreadKey`]: `crate::ThreadKey`
	pub fn lock<'s, 'key: 's, Key: Keyable + 'key>(
		&'s self,
		key: Key,
	) -> RwLockReadGuard<'s, 'key, T, Key, R> {
		self.0.read(key)
	}

	/// Attempts to acquire the underlying [`RwLock`] with shared read access
	/// without blocking.
	///
	/// If the access could not be granted at this time, then `Err` is
	/// returned. Otherwise, an RAII guard is returned which will release the
	/// shared access when it is dropped.
	///
	/// This function does not provide any guarantees with respect to the
	/// ordering of whether contentious readers or writers will acquire the
	/// lock first.
	///
	/// # Errors
	///
	/// If the `RwLock` could not be acquired because it was already locked
	/// exclusively, then an error will be returned containing the given key.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey};
	/// use happylock::rwlock::ReadLock;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = RwLock::new(1);
	/// let reader = ReadLock::new(&lock);
	///
	/// match reader.try_lock(key) {
	///     Ok(n) => assert_eq!(*n, 1),
	///     Err(_) => unreachable!(),
	/// };
	/// ```
	pub fn try_lock<'s, 'key: 's, Key: Keyable + 'key>(
		&'s self,
		key: Key,
	) -> Result<RwLockReadGuard<'s, 'key, T, Key, R>, Key> {
		self.0.try_read(key)
	}

	/// Attempts to create an exclusive lock without a key. Locking this
	/// without exclusive access to the key is undefined behavior.
	pub(crate) unsafe fn try_lock_no_key(&self) -> Option<RwLockReadRef<'_, T, R>> {
		self.0.try_read_no_key()
	}

	/// Immediately drops the guard, and consequently releases the shared lock
	/// on the underlying [`RwLock`].
	///
	/// This function is equivalent to calling [`drop`] on the guard, except
	/// that it returns the key that was used to create it. Alternately, the
	/// guard will be automatically dropped when it goes out of scope.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey};
	/// use happylock::rwlock::ReadLock;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = RwLock::new(0);
	/// let reader = ReadLock::new(&lock);
	///
	/// let mut guard = reader.lock(key);
	/// assert_eq!(*guard, 0);
	/// let key = ReadLock::unlock(guard);
	/// ```
	pub fn unlock<'key, Key: Keyable + 'key>(guard: RwLockReadGuard<'_, 'key, T, Key, R>) -> Key {
		RwLock::unlock_read(guard)
	}
}
