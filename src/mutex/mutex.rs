use std::fmt::Debug;
use std::{cell::UnsafeCell, marker::PhantomData};

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
	pub const fn new(data: T) -> Self {
		Self {
			raw: R::INIT,
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

impl<T: ?Sized + Default, R: RawMutex> Default for Mutex<T, R> {
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
// This is safe because you can't have a mutable reference to the lock if it's
// locked. Being locked requires an immutable reference because of the guard.
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
	///     let key = ThreadKey::get().unwrap();
	///     *c_mutex.lock(key) = 10;
	/// }).join().expect("thread::spawn failed");
	///
	/// let key = ThreadKey::get().unwrap();
	/// assert_eq!(*mutex.lock(key), 10);
	/// ```
	pub fn lock<'s, 'k: 's, Key: Keyable>(&'s self, key: Key) -> MutexGuard<'_, 'k, T, Key, R> {
		unsafe {
			self.raw.lock();

			// safety: we just locked the mutex
			MutexGuard::new(self, key)
		}
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
	///     let key = ThreadKey::get().unwrap();
	///     let mut lock = c_mutex.try_lock(key);
	///     if let Some(mut lock) = lock {
	///         *lock = 10;
	///     } else {
	///         println!("try_lock failed");
	///     }
	/// }).join().expect("thread::spawn failed");
	///
	/// let key = ThreadKey::get().unwrap();
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
		self.raw.try_lock().then_some(MutexRef(self, PhantomData))
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
	/// let key = ThreadKey::get().unwrap();
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
unsafe impl<R: RawMutex + Sync, T: ?Sized + Send> Sync for Mutex<T, R> {}
