use std::cell::UnsafeCell;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::panic::AssertUnwindSafe;

use lock_api::RawRwLock;

use crate::collection::utils;
use crate::handle_unwind::handle_unwind;
use crate::lockable::{
	Lockable, LockableGetMut, LockableIntoInner, OwnedLockable, RawLock, Sharable,
};
use crate::{Keyable, ThreadKey};

use super::{PoisonFlag, RwLock, RwLockReadGuard, RwLockReadRef, RwLockWriteGuard, RwLockWriteRef};

unsafe impl<T: ?Sized, R: RawRwLock> RawLock for RwLock<T, R> {
	fn poison(&self) {
		self.poison.poison();
	}

	unsafe fn raw_write(&self) {
		assert!(
			!self.poison.is_poisoned(),
			"The read-write lock has been killed"
		);

		// if the closure unwraps, then the mutex will be killed
		let this = AssertUnwindSafe(self);
		handle_unwind(|| this.raw.lock_exclusive(), || self.poison())
	}

	unsafe fn raw_try_write(&self) -> bool {
		if self.poison.is_poisoned() {
			return false;
		}

		// if the closure unwraps, then the mutex will be killed
		let this = AssertUnwindSafe(self);
		handle_unwind(|| this.raw.try_lock_exclusive(), || self.poison())
	}

	unsafe fn raw_unlock_write(&self) {
		// if the closure unwraps, then the mutex will be killed
		let this = AssertUnwindSafe(self);
		handle_unwind(|| this.raw.unlock_exclusive(), || self.poison())
	}

	unsafe fn raw_read(&self) {
		assert!(
			!self.poison.is_poisoned(),
			"The read-write lock has been killed"
		);

		// if the closure unwraps, then the mutex will be killed
		let this = AssertUnwindSafe(self);
		handle_unwind(|| this.raw.lock_shared(), || self.poison())
	}

	unsafe fn raw_try_read(&self) -> bool {
		if self.poison.is_poisoned() {
			return false;
		}

		// if the closure unwraps, then the mutex will be killed
		let this = AssertUnwindSafe(self);
		handle_unwind(|| this.raw.try_lock_shared(), || self.poison())
	}

	unsafe fn raw_unlock_read(&self) {
		// if the closure unwraps, then the mutex will be killed
		let this = AssertUnwindSafe(self);
		handle_unwind(|| this.raw.unlock_shared(), || self.poison())
	}
}

unsafe impl<T, R: RawRwLock> Lockable for RwLock<T, R> {
	type Guard<'g>
		= RwLockWriteRef<'g, T, R>
	where
		Self: 'g;

	type DataMut<'a>
		= &'a mut T
	where
		Self: 'a;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		ptrs.push(self);
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		RwLockWriteRef::new(self)
	}

	unsafe fn data_mut(&self) -> Self::DataMut<'_> {
		self.data.get().as_mut().unwrap_unchecked()
	}
}

unsafe impl<T, R: RawRwLock> Sharable for RwLock<T, R> {
	type ReadGuard<'g>
		= RwLockReadRef<'g, T, R>
	where
		Self: 'g;

	type DataRef<'a>
		= &'a T
	where
		Self: 'a;

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		RwLockReadRef::new(self)
	}

	unsafe fn data_ref(&self) -> Self::DataRef<'_> {
		self.data.get().as_ref().unwrap_unchecked()
	}
}

unsafe impl<T: Send, R: RawRwLock> OwnedLockable for RwLock<T, R> {}

impl<T: Send, R: RawRwLock> LockableIntoInner for RwLock<T, R> {
	type Inner = T;

	fn into_inner(self) -> Self::Inner {
		self.into_inner()
	}
}

impl<T: Send, R: RawRwLock> LockableGetMut for RwLock<T, R> {
	type Inner<'a>
		= &'a mut T
	where
		Self: 'a;

	fn get_mut(&mut self) -> Self::Inner<'_> {
		AsMut::as_mut(self)
	}
}

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
			poison: PoisonFlag::new(),
			raw: R::INIT,
		}
	}
}

#[mutants::skip]
#[cfg(not(tarpaulin_include))]
impl<T: Debug, R: RawRwLock> Debug for RwLock<T, R> {
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

impl<T: Default, R: RawRwLock> Default for RwLock<T, R> {
	fn default() -> Self {
		Self::new(T::default())
	}
}

impl<T, R: RawRwLock> From<T> for RwLock<T, R> {
	fn from(value: T) -> Self {
		Self::new(value)
	}
}

// We don't need a `get_mut` because we don't have mutex poisoning. Hurray!
// This is safe because you can't have a mutable reference to the lock if it's
// locked. Being locked requires an immutable reference because of the guard.
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

impl<T: ?Sized, R> RwLock<T, R> {
	/// Returns a mutable reference to the underlying data.
	///
	/// Since this call borrows `RwLock` mutably, no actual locking is taking
	/// place. The mutable borrow statically guarantees that no locks exist.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{ThreadKey, RwLock};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let mut mutex = RwLock::new(0);
	/// *mutex.get_mut() = 10;
	/// assert_eq!(*mutex.read(key), 10);
	/// ```
	#[must_use]
	pub fn get_mut(&mut self) -> &mut T {
		self.data.get_mut()
	}
}

impl<T, R: RawRwLock> RwLock<T, R> {
	pub fn scoped_read<'a, Ret>(&'a self, key: impl Keyable, f: impl Fn(&'a T) -> Ret) -> Ret {
		utils::scoped_read(self, key, f)
	}

	pub fn scoped_try_read<'a, Key: Keyable, Ret>(
		&'a self,
		key: Key,
		f: impl Fn(&'a T) -> Ret,
	) -> Result<Ret, Key> {
		utils::scoped_try_read(self, key, f)
	}

	pub fn scoped_write<'a, Ret>(&'a self, key: impl Keyable, f: impl Fn(&'a mut T) -> Ret) -> Ret {
		utils::scoped_write(self, key, f)
	}

	pub fn scoped_try_write<'a, Key: Keyable, Ret>(
		&'a self,
		key: Key,
		f: impl Fn(&'a mut T) -> Ret,
	) -> Result<Ret, Key> {
		utils::scoped_try_write(self, key, f)
	}

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
	pub fn read(&self, key: ThreadKey) -> RwLockReadGuard<'_, T, R> {
		unsafe {
			self.raw_read();

			// safety: the lock is locked first
			RwLockReadGuard::new(self, key)
		}
	}

	/// Attempts to acquire this `RwLock` with shared read access without
	/// blocking.
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
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = RwLock::new(1);
	///
	/// match lock.try_read(key) {
	///     Ok(n) => assert_eq!(*n, 1),
	///     Err(_) => unreachable!(),
	/// };
	/// ```
	pub fn try_read(&self, key: ThreadKey) -> Result<RwLockReadGuard<'_, T, R>, ThreadKey> {
		unsafe {
			if self.raw_try_read() {
				// safety: the lock is locked first
				Ok(RwLockReadGuard::new(self, key))
			} else {
				Err(key)
			}
		}
	}

	/// Attempts to create a shared lock without a key. Locking this without
	/// exclusive access to the key is undefined behavior.
	pub(crate) unsafe fn try_read_no_key(&self) -> Option<RwLockReadRef<'_, T, R>> {
		if self.raw_try_read() {
			// safety: the lock is locked first
			Some(RwLockReadRef(self, PhantomData))
		} else {
			None
		}
	}

	/// Attempts to create an exclusive lock without a key. Locking this
	/// without exclusive access to the key is undefined behavior.
	#[cfg(test)]
	pub(crate) unsafe fn try_write_no_key(&self) -> Option<RwLockWriteRef<'_, T, R>> {
		if self.raw_try_write() {
			// safety: the lock is locked first
			Some(RwLockWriteRef(self, PhantomData))
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
	/// match lock.try_write(key) {
	///     Ok(n) => assert_eq!(*n, 1),
	///     Err(_) => unreachable!(),
	/// };
	/// ```
	///
	/// [`ThreadKey`]: `crate::ThreadKey`
	pub fn write(&self, key: ThreadKey) -> RwLockWriteGuard<'_, T, R> {
		unsafe {
			self.raw_write();

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
	/// # Errors
	///
	/// If the `RwLock` could not be acquired because it was already locked,
	/// then an error will be returned containing the given key.
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
	pub fn try_write(&self, key: ThreadKey) -> Result<RwLockWriteGuard<'_, T, R>, ThreadKey> {
		unsafe {
			if self.raw_try_write() {
				// safety: the lock is locked first
				Ok(RwLockWriteGuard::new(self, key))
			} else {
				Err(key)
			}
		}
	}

	/// Returns `true` if the rwlock is currently locked in any way
	#[cfg(test)]
	pub(crate) fn is_locked(&self) -> bool {
		self.raw.is_locked()
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
	#[must_use]
	pub fn unlock_read(guard: RwLockReadGuard<'_, T, R>) -> ThreadKey {
		drop(guard.rwlock);
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
	#[must_use]
	pub fn unlock_write(guard: RwLockWriteGuard<'_, T, R>) -> ThreadKey {
		drop(guard.rwlock);
		guard.thread_key
	}
}

unsafe impl<R: RawRwLock + Send, T: ?Sized + Send> Send for RwLock<T, R> {}
unsafe impl<R: RawRwLock + Sync, T: ?Sized + Send> Sync for RwLock<T, R> {}
