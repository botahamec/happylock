use std::cell::UnsafeCell;
use std::fmt::Debug;

use lock_api::RawMutex;

use crate::key::Keyable;

use super::{Mutex, MutexGuard, MutexRef};

impl<T, R: RawMutex> Mutex<T, R> {
	/// Create a new unlocked `Mutex`.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::Mutex;
	///
	/// let mutex = Mutex::new(0);
	/// ```
	#[must_use]
	pub const fn new(value: T) -> Self {
		Self {
			raw: R::INIT,
			value: UnsafeCell::new(value),
		}
	}
}

impl<T: ?Sized, R> Debug for Mutex<T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&format!("Mutex<{}>", std::any::type_name::<T>()))
	}
}

impl<T, R: RawMutex> From<T> for Mutex<T, R> {
	fn from(value: T) -> Self {
		Self::new(value)
	}
}

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
		self.value.into_inner()
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
	/// let key = ThreadKey::lock().unwrap();
	/// let mut mutex = Mutex::new(0);
	/// *mutex.get_mut() = 10;
	/// assert_eq!(*mutex.lock(key), 10);
	/// ```
	#[must_use]
	pub fn get_mut(&mut self) -> &mut T {
		self.value.get_mut()
	}
}

impl<T: ?Sized, R: RawMutex> Mutex<T, R> {
	/// Block the thread until this mutex can be locked, and lock it.
	///
	/// Upon returning, the thread is the only thread with a lock on the
	/// `Mutex`. A [`MutexGuard`] is returned to allow a scoped unlock of this
	/// `Mutex`. When the guard is dropped, this `Mutex` will unlock.
	///
	/// # Examples
	///
	/// ```
	/// use std::{thread, sync::Arc};
	/// use happylock::{Mutex, ThreadKey};
	///
	/// let mutex = Arc::new(Mutex::new(0));
	/// let c_mutex = Arc::clone(&mutex);
	///
	/// thread::spawn(move || {
	///     let key = ThreadKey::lock().unwrap();
	///     *c_mutex.lock(key) = 10;
	/// }).join().expect("thread::spawn failed");
	///
	/// let key = ThreadKey::lock().unwrap();
	/// assert_eq!(*mutex.lock(key), 10);
	/// ```
	pub fn lock<'s, 'k: 's, Key: Keyable>(&'s self, key: Key) -> MutexGuard<'_, 'k, T, Key, R> {
		unsafe {
			self.raw.lock();

			// safety: we just locked the mutex
			MutexGuard::new(self, key)
		}
	}

	/// Lock without a [`ThreadKey`]. You must exclusively own the
	/// [`ThreadKey`] as long as the [`MutexRef`] is alive. This may cause
	/// deadlock if called multiple times without unlocking first.
	pub(crate) unsafe fn lock_no_key(&self) -> MutexRef<'_, T, R> {
		self.raw.lock();

		MutexRef(self)
	}

	/// Attempts to lock the `Mutex` without blocking.
	///
	/// # Errors
	///
	/// Returns [`Err`] if the `Mutex` cannot be locked without blocking.
	///
	/// # Examples
	///
	/// ```
	/// use std::{thread, sync::Arc};
	/// use happylock::{Mutex, ThreadKey};
	///
	/// let mutex = Arc::new(Mutex::new(0));
	/// let c_mutex = Arc::clone(&mutex);
	///
	/// thread::spawn(move || {
	///     let key = ThreadKey::lock().unwrap();
	///     let mut lock = c_mutex.try_lock(key);
	///     if let Some(mut lock) = lock {
	///         *lock = 10;
	///     } else {
	///         println!("try_lock failed");
	///     }
	/// }).join().expect("thread::spawn failed");
	///
	/// let key = ThreadKey::lock().unwrap();
	/// assert_eq!(*mutex.lock(key), 10);
	/// ```
	pub fn try_lock<'s, 'a: 's, 'k: 'a, Key: Keyable>(
		&'s self,
		key: Key,
	) -> Option<MutexGuard<'_, 'k, T, Key, R>> {
		if self.raw.try_lock() {
			// safety: we just locked the mutex
			Some(unsafe { MutexGuard::new(self, key) })
		} else {
			None
		}
	}

	/// Lock without a [`ThreadKey`]. It is undefined behavior to do this without
	/// owning the [`ThreadKey`].
	pub(crate) unsafe fn try_lock_no_key(&self) -> Option<MutexRef<'_, T, R>> {
		self.raw.try_lock().then_some(MutexRef(self))
	}

	/// Forcibly unlocks the `Lock`.
	///
	/// # Safety
	///
	/// This should only be called if there are no references to any
	/// [`MutexGuard`]s for this mutex in the program.
	pub(super) unsafe fn force_unlock(&self) {
		self.raw.unlock();
	}

	/// Consumes the [`MutexGuard`], and consequently unlocks its `Mutex`.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{ThreadKey, Mutex};
	///
	/// let key = ThreadKey::lock().unwrap();
	/// let mutex = Mutex::new(0);
	///
	/// let mut guard = mutex.lock(key);
	/// *guard += 20;
	///
	/// let key = Mutex::unlock(guard);
	/// ```
	pub fn unlock<'a, 'k: 'a, Key: Keyable + 'k>(guard: MutexGuard<'a, 'k, T, Key, R>) -> Key {
		unsafe {
			guard.mutex.0.force_unlock();
		}
		guard.thread_key
	}
}

unsafe impl<R: RawMutex + Send, T: ?Sized + Send> Send for Mutex<T, R> {}
unsafe impl<R: RawMutex + Sync, T: ?Sized + Send + Sync> Sync for Mutex<T, R> {}
