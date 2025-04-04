use std::cell::UnsafeCell;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::panic::AssertUnwindSafe;

use lock_api::RawMutex;

use crate::handle_unwind::handle_unwind;
use crate::lockable::{Lockable, LockableGetMut, LockableIntoInner, OwnedLockable, RawLock};
use crate::poisonable::PoisonFlag;
use crate::{Keyable, ThreadKey};

use super::{Mutex, MutexGuard, MutexRef};

unsafe impl<T: ?Sized, R: RawMutex> RawLock for Mutex<T, R> {
	fn poison(&self) {
		self.poison.poison();
	}

	unsafe fn raw_write(&self) {
		assert!(!self.poison.is_poisoned(), "The mutex has been killed");

		// if the closure unwraps, then the mutex will be killed
		let this = AssertUnwindSafe(self);
		handle_unwind(|| this.raw.lock(), || self.poison())
	}

	unsafe fn raw_try_write(&self) -> bool {
		if self.poison.is_poisoned() {
			return false;
		}

		// if the closure unwraps, then the mutex will be killed
		let this = AssertUnwindSafe(self);
		handle_unwind(|| this.raw.try_lock(), || self.poison())
	}

	unsafe fn raw_unlock_write(&self) {
		// if the closure unwraps, then the mutex will be killed
		let this = AssertUnwindSafe(self);
		handle_unwind(|| this.raw.unlock(), || self.poison())
	}

	// this is the closest thing to a read we can get, but Sharable isn't
	// implemented for this
	#[mutants::skip]
	#[cfg(not(tarpaulin_include))]
	unsafe fn raw_read(&self) {
		self.raw_write()
	}

	#[mutants::skip]
	#[cfg(not(tarpaulin_include))]
	unsafe fn raw_try_read(&self) -> bool {
		self.raw_try_write()
	}

	#[mutants::skip]
	#[cfg(not(tarpaulin_include))]
	unsafe fn raw_unlock_read(&self) {
		self.raw_unlock_write()
	}
}

unsafe impl<T, R: RawMutex> Lockable for Mutex<T, R> {
	type Guard<'g>
		= MutexRef<'g, T, R>
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
		MutexRef::new(self)
	}

	unsafe fn data_mut(&self) -> Self::DataMut<'_> {
		self.data.get().as_mut().unwrap_unchecked()
	}
}

impl<T, R: RawMutex> LockableIntoInner for Mutex<T, R> {
	type Inner = T;

	fn into_inner(self) -> Self::Inner {
		self.into_inner()
	}
}

impl<T, R: RawMutex> LockableGetMut for Mutex<T, R> {
	type Inner<'a>
		= &'a mut T
	where
		Self: 'a;

	fn get_mut(&mut self) -> Self::Inner<'_> {
		self.get_mut()
	}
}

unsafe impl<T, R: RawMutex> OwnedLockable for Mutex<T, R> {}

impl<T, R: RawMutex> Mutex<T, R> {
	/// Creates a `Mutex` in an unlocked state ready for use.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::Mutex;
	///
	/// let mutex = Mutex::new(0);
	/// ```
	#[must_use]
	pub const fn new(data: T) -> Self {
		Self {
			raw: R::INIT,
			poison: PoisonFlag::new(),
			data: UnsafeCell::new(data),
		}
	}

	/// Returns the raw underlying mutex.
	///
	/// Note that you will most likely need to import the [`RawMutex`] trait
	/// from `lock_api` to be able to call functions on the raw mutex.
	///
	/// # Safety
	///
	/// This method is unsafe because it allows unlocking a mutex while still
	/// holding a reference to a [`MutexGuard`], and locking a mutex without
	/// holding the [`ThreadKey`].
	///
	/// [`ThreadKey`]: `crate::ThreadKey`
	#[must_use]
	pub const unsafe fn raw(&self) -> &R {
		&self.raw
	}
}

#[mutants::skip]
#[cfg(not(tarpaulin_include))]
impl<T: ?Sized + Debug, R: RawMutex> Debug for Mutex<T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		// safety: this is just a try lock, and the value is dropped
		//         immediately after, so there's no risk of blocking ourselves
		//         or any other threads
		// when i implement try_clone this code will become less unsafe
		if let Some(value) = unsafe { self.try_lock_no_key() } {
			f.debug_struct("Mutex").field("data", &&*value).finish()
		} else {
			struct LockedPlaceholder;
			impl Debug for LockedPlaceholder {
				fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
					f.write_str("<locked>")
				}
			}

			f.debug_struct("Mutex")
				.field("data", &LockedPlaceholder)
				.finish()
		}
	}
}

impl<T: Default, R: RawMutex> Default for Mutex<T, R> {
	fn default() -> Self {
		Self::new(T::default())
	}
}

impl<T, R: RawMutex> From<T> for Mutex<T, R> {
	fn from(value: T) -> Self {
		Self::new(value)
	}
}

// We don't need a `get_mut` because we don't have mutex poisoning. Hurray!
// We have it anyway for documentation
impl<T: ?Sized, R> AsMut<T> for Mutex<T, R> {
	fn as_mut(&mut self) -> &mut T {
		self.get_mut()
	}
}

impl<T, R> Mutex<T, R> {
	/// Consumes this mutex, returning the underlying data.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::Mutex;
	///
	/// let mutex = Mutex::new(0);
	/// assert_eq!(mutex.into_inner(), 0);
	/// ```
	#[must_use]
	pub fn into_inner(self) -> T {
		self.data.into_inner()
	}
}

impl<T: ?Sized, R> Mutex<T, R> {
	/// Returns a mutable reference to the underlying data.
	///
	/// Since this call borrows `Mutex` mutably, no actual locking is taking
	/// place. The mutable borrow statically guarantees that no locks exist.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{ThreadKey, Mutex};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let mut mutex = Mutex::new(0);
	/// *mutex.get_mut() = 10;
	/// assert_eq!(*mutex.lock(key), 10);
	/// ```
	#[must_use]
	pub fn get_mut(&mut self) -> &mut T {
		self.data.get_mut()
	}
}

impl<T: ?Sized, R: RawMutex> Mutex<T, R> {
	pub fn scoped_lock<'a, Ret>(
		&'a self,
		key: impl Keyable,
		f: impl FnOnce(&'a mut T) -> Ret,
	) -> Ret {
		unsafe {
			// safety: we have the key
			self.raw_write();

			// safety: the data has been locked
			let r = handle_unwind(
				|| f(self.data.get().as_mut().unwrap_unchecked()),
				|| self.raw_unlock_write(),
			);

			// ensures the key is held long enough
			drop(key);

			// safety: the mutex is still locked
			self.raw_unlock_write();

			r
		}
	}

	pub fn scoped_try_lock<'a, Key: Keyable, Ret>(
		&'a self,
		key: Key,
		f: impl FnOnce(&'a mut T) -> Ret,
	) -> Result<Ret, Key> {
		unsafe {
			// safety: we have the key
			if !self.raw_try_write() {
				return Err(key);
			}

			// safety: the data has been locked
			let r = handle_unwind(
				|| f(self.data.get().as_mut().unwrap_unchecked()),
				|| self.raw_unlock_write(),
			);

			// ensures the key is held long enough
			drop(key);

			// safety: the mutex is still locked
			self.raw_unlock_write();

			Ok(r)
		}
	}
}

impl<T: ?Sized, R: RawMutex> Mutex<T, R> {
	/// Acquires a mutex, blocking the current thread until it is able to do so.
	///
	/// This function will block the local thread until it is available to acquire
	/// the mutex. Upon returning, the thread is the only thread with the lock
	/// held. A [`MutexGuard`] is returned to allow a scoped unlock of this
	/// `Mutex`. When the guard goes out of scope, this `Mutex` will unlock.
	///
	/// Due to the requirement of a [`ThreadKey`] to call this function, it is not
	/// possible for this function to deadlock.
	///
	/// # Examples
	///
	/// ```
	/// use std::thread;
	/// use happylock::{Mutex, ThreadKey};
	///
	/// let mutex = Mutex::new(0);
	///
	/// thread::scope(|s| {
	///     s.spawn(|| {
	///         let key = ThreadKey::get().unwrap();
	///         *mutex.lock(key) = 10;
	///     });
	/// });
	///
	/// let key = ThreadKey::get().unwrap();
	/// assert_eq!(*mutex.lock(key), 10);
	/// ```
	pub fn lock(&self, key: ThreadKey) -> MutexGuard<'_, T, R> {
		unsafe {
			// safety: we have the thread key
			self.raw_write();

			// safety: we just locked the mutex
			MutexGuard::new(self, key)
		}
	}

	/// Attempts to lock this `Mutex` without blocking.
	///
	/// If the lock could not be acquired at this time, then `Err` is returned.
	/// Otherwise, an RAII guard is returned. The lock will be unlocked when the
	/// guard is dropped.
	///
	/// # Errors
	///
	/// If the mutex could not be acquired because it is already locked, then
	/// this call will return an error containing the [`ThreadKey`].
	///
	/// # Examples
	///
	/// ```
	/// use std::thread;
	/// use happylock::{Mutex, ThreadKey};
	///
	/// let mutex = Mutex::new(0);
	///
	/// thread::scope(|s| {
	///     s.spawn(|| {
	///         let key = ThreadKey::get().unwrap();
	///         let mut lock = mutex.try_lock(key);
	///         if let Ok(mut lock) = lock {
	///             *lock = 10;
	///         } else {
	///             println!("try_lock failed");
	///         }
	///     });
	/// });
	///
	/// let key = ThreadKey::get().unwrap();
	/// assert_eq!(*mutex.lock(key), 10);
	/// ```
	pub fn try_lock(&self, key: ThreadKey) -> Result<MutexGuard<'_, T, R>, ThreadKey> {
		unsafe {
			// safety: we have the key to the mutex
			if self.raw_try_write() {
				// safety: we just locked the mutex
				Ok(MutexGuard::new(self, key))
			} else {
				Err(key)
			}
		}
	}

	/// Returns `true` if the mutex is currently locked
	#[cfg(test)]
	pub(crate) fn is_locked(&self) -> bool {
		self.raw.is_locked()
	}

	/// Lock without a [`ThreadKey`]. It is undefined behavior to do this without
	/// owning the [`ThreadKey`].
	pub(crate) unsafe fn try_lock_no_key(&self) -> Option<MutexRef<'_, T, R>> {
		self.raw_try_write().then_some(MutexRef(self, PhantomData))
	}

	/// Consumes the [`MutexGuard`], and consequently unlocks its `Mutex`.
	///
	/// This function is equivalent to calling [`drop`] on the guard, except that
	/// it returns the key that was used to create it. Alernatively, the guard
	/// will be automatically dropped when it goes out of scope.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{ThreadKey, Mutex};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let mutex = Mutex::new(0);
	///
	/// let mut guard = mutex.lock(key);
	/// *guard += 20;
	///
	/// let key = Mutex::unlock(guard);
	///
	/// let guard = mutex.lock(key);
	/// assert_eq!(*guard, 20);
	/// ```
	#[must_use]
	pub fn unlock(guard: MutexGuard<'_, T, R>) -> ThreadKey {
		drop(guard.mutex);
		guard.thread_key
	}
}

unsafe impl<R: RawMutex + Send, T: ?Sized + Send> Send for Mutex<T, R> {}
unsafe impl<R: RawMutex + Sync, T: ?Sized + Send> Sync for Mutex<T, R> {}
