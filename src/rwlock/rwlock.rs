use std::fmt::Debug;
use std::{cell::UnsafeCell, marker::PhantomData};

use lock_api::RawRwLock;

use crate::key::Keyable;

use super::{RwLock, RwLockReadGuard, RwLockReadRef, RwLockWriteGuard};

impl<T, R: RawRwLock> RwLock<T, R> {
	/// Creates a new instance of an `RwLock<T>` which is unlocked.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::RwLock;
	///
	/// let lock = RwLock::new(5);
	/// ```
	#[must_use]
	pub const fn new(data: T) -> Self {
		Self {
			data: UnsafeCell::new(data),
			raw: R::INIT,
		}
	}

	/// Returns the underlying raw reader-writer lock object.
	///
	/// Note that you will most likely need to import the [`RawRwLock`] trait
	/// from `lock_api` to be able to call functions on the raw reader-writer
	/// lock.
	///
	/// # Safety
	///
	/// This method is unsafe because it allows unlocking a mutex while
	/// still holding a reference to a lock guard.
	pub const unsafe fn raw(&self) -> &R {
		&self.raw
	}
}

impl<T: ?Sized + Debug, R: RawRwLock> Debug for RwLock<T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		// safety: this is just a try lock, and the value is dropped
		//         immediately after, so there's no risk of blocking ourselves
		//         or any other threads
		if let Some(value) = unsafe { self.try_read_no_key() } {
			f.debug_struct("RwLock").field("data", &&*value).finish()
		} else {
			struct LockedPlaceholder;
			impl Debug for LockedPlaceholder {
				fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
					f.write_str("<locked>")
				}
			}

			f.debug_struct("RwLock")
				.field("data", &LockedPlaceholder)
				.finish()
		}
	}
}

impl<T: ?Sized + Default, R: RawRwLock> Default for RwLock<T, R> {
	fn default() -> Self {
		Self::new(T::default())
	}
}

impl<T, R: RawRwLock> From<T> for RwLock<T, R> {
	fn from(value: T) -> Self {
		Self::new(value)
	}
}

impl<T: ?Sized, R> AsMut<T> for RwLock<T, R> {
	fn as_mut(&mut self) -> &mut T {
		self.data.get_mut()
	}
}

impl<T, R> RwLock<T, R> {
	/// Consumes this `RwLock`, returning the underlying data.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey};
	///
	/// let lock = RwLock::new(String::new());
	/// {
	///     let key = ThreadKey::get().unwrap();
	///     let mut s = lock.write(key);
	///     *s = "modified".to_owned();
	/// }
	/// assert_eq!(lock.into_inner(), "modified");
	/// ```
	#[must_use]
	pub fn into_inner(self) -> T {
		self.data.into_inner()
	}
}

impl<T: ?Sized, R: RawRwLock> RwLock<T, R> {
	/// Locks this `RwLock` with shared read access, blocking the current
	/// thread until it can be acquired.
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
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = Arc::new(RwLock::new(1));
	/// let c_lock = Arc::clone(&lock);
	///
	/// let n = lock.read(key);
	/// assert_eq!(*n, 1);
	///
	/// thread::spawn(move || {
	///     let key = ThreadKey::get().unwrap();
	///     let r = c_lock.read(key);
	/// }).join().unwrap();
	/// ```
	///
	/// [`ThreadKey`]: `crate::ThreadKey`
	pub fn read<'s, 'key: 's, Key: Keyable>(
		&'s self,
		key: Key,
	) -> RwLockReadGuard<'_, 'key, T, Key, R> {
		unsafe {
			self.raw.lock_shared();

			// safety: the lock is locked first
			RwLockReadGuard::new(self, key)
		}
	}

	/// Attempts to acquire this `RwLock` with shared read access without
	/// blocking.
	///
	/// If the access could not be granted at this time, then `None` is
	/// returned. Otherwise, an RAII guard is returned which will release the
	/// shared access when it is dropped.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = RwLock::new(1);
	///
	/// match lock.try_read(key) {
	///     Some(n) => assert_eq!(*n, 1),
	///     None => unreachable!(),
	/// };
	/// ```
	pub fn try_read<'s, 'key: 's, Key: Keyable>(
		&'s self,
		key: Key,
	) -> Option<RwLockReadGuard<'_, 'key, T, Key, R>> {
		unsafe {
			if self.raw.try_lock_shared() {
				// safety: the lock is locked first
				Some(RwLockReadGuard::new(self, key))
			} else {
				None
			}
		}
	}

	/// Attempts to create a shared lock without a key. Locking this without
	/// exclusive access to the key is undefined behavior.
	pub(crate) unsafe fn try_read_no_key(&self) -> Option<RwLockReadRef<'_, T, R>> {
		if self.raw.try_lock_shared() {
			// safety: the lock is locked first
			Some(RwLockReadRef(self, PhantomData))
		} else {
			None
		}
	}

	/// Locks this `RwLock` with exclusive write access, blocking the current
	/// until it can be acquired.
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
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = RwLock::new(1);
	///
	/// let mut n = lock.write(key);
	/// *n += 2;
	/// ```
	///
	/// [`ThreadKey`]: `crate::ThreadKey`
	pub fn write<'s, 'key: 's, Key: Keyable>(
		&'s self,
		key: Key,
	) -> RwLockWriteGuard<'_, 'key, T, Key, R> {
		unsafe {
			self.raw.lock_exclusive();

			// safety: the lock is locked first
			RwLockWriteGuard::new(self, key)
		}
	}

	/// Attempts to lock this `RwLock` with exclusive write access.
	///
	/// This function does not block. If the lock could not be acquired at this
	/// time, then `None` is returned. Otherwise, an RAII guard is returned
	/// which will release the lock when it is dropped.
	///
	/// This function does not provide any guarantees with respect to the
	/// ordering of whether contentious readers or writers will acquire the
	/// lock first.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = RwLock::new(1);
	///
	/// let n = lock.read(key);
	/// assert_eq!(*n, 1);
	/// ```
	pub fn try_write<'s, 'key: 's, Key: Keyable>(
		&'s self,
		key: Key,
	) -> Option<RwLockWriteGuard<'_, 'key, T, Key, R>> {
		unsafe {
			if self.raw.try_lock_exclusive() {
				// safety: the lock is locked first
				Some(RwLockWriteGuard::new(self, key))
			} else {
				None
			}
		}
	}

	/// Unlocks shared access on the `RwLock`. This is undefined behavior is
	/// the data is still accessible.
	pub(super) unsafe fn force_unlock_read(&self) {
		self.raw.unlock_shared();
	}

	/// Unlocks exclusive access on the `RwLock`. This is undefined behavior is
	/// the data is still accessible.
	pub(super) unsafe fn force_unlock_write(&self) {
		self.raw.unlock_exclusive();
	}

	/// Immediately drops the guard, and consequently releases the shared lock.
	///
	/// This function is equivalent to calling [`drop`] on the guard, except
	/// that it returns the key that was used to create it. Alternately, the
	/// guard will be automatically dropped when it goes out of scope.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = RwLock::new(0);
	///
	/// let mut guard = lock.read(key);
	/// assert_eq!(*guard, 0);
	/// let key = RwLock::unlock_read(guard);
	/// ```
	pub fn unlock_read<'key, Key: Keyable + 'key>(
		guard: RwLockReadGuard<'_, 'key, T, Key, R>,
	) -> Key {
		unsafe {
			guard.rwlock.0.force_unlock_read();
		}
		guard.thread_key
	}

	/// Immediately drops the guard, and consequently releases the exclusive
	/// lock.
	///
	/// This function is equivalent to calling [`drop`] on the guard, except
	/// that it returns the key that was used to create it. Alternately, the
	/// guard will be automatically dropped when it goes out of scope.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = RwLock::new(0);
	///
	/// let mut guard = lock.write(key);
	/// *guard += 20;
	/// let key = RwLock::unlock_write(guard);
	/// ```
	pub fn unlock_write<'key, Key: Keyable + 'key>(
		guard: RwLockWriteGuard<'_, 'key, T, Key, R>,
	) -> Key {
		unsafe {
			guard.rwlock.0.force_unlock_write();
		}
		guard.thread_key
	}
}

unsafe impl<R: RawRwLock + Send, T: ?Sized + Send> Send for RwLock<T, R> {}
unsafe impl<R: RawRwLock + Sync, T: ?Sized + Send> Sync for RwLock<T, R> {}
