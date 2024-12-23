use std::cell::UnsafeCell;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::panic::AssertUnwindSafe;

use lock_api::RawMutex;

use crate::handle_unwind::handle_unwind;
use crate::key::Keyable;
use crate::lockable::{Lockable, LockableAsMut, LockableIntoInner, OwnedLockable, RawLock};
use crate::poisonable::PoisonFlag;

use super::{Mutex, MutexGuard, MutexRef};

unsafe impl<T: ?Sized, R: RawMutex> RawLock for Mutex<T, R> {
	fn kill(&self) {
		self.poison.poison();
	}

	unsafe fn raw_lock(&self) {
		assert!(!self.poison.is_poisoned(), "The mutex has been killed");

		// if the closure unwraps, then the mutex will be killed
		let this = AssertUnwindSafe(self);
		handle_unwind(|| this.raw.lock(), || self.kill())
	}

	unsafe fn raw_try_lock(&self) -> bool {
		if self.poison.is_poisoned() {
			return false;
		}

		// if the closure unwraps, then the mutex will be killed
		let this = AssertUnwindSafe(self);
		handle_unwind(|| this.raw.try_lock(), || self.kill())
	}

	unsafe fn raw_unlock(&self) {
		// if the closure unwraps, then the mutex will be killed
		let this = AssertUnwindSafe(self);
		handle_unwind(|| this.raw.unlock(), || self.kill())
	}

	// this is the closest thing to a read we can get, but Sharable isn't
	// implemented for this
	unsafe fn raw_read(&self) {
		self.raw_lock()
	}

	unsafe fn raw_try_read(&self) -> bool {
		self.raw_try_lock()
	}

	unsafe fn raw_unlock_read(&self) {
		self.raw_unlock()
	}
}

unsafe impl<T: Send, R: RawMutex + Send + Sync> Lockable for Mutex<T, R> {
	type Guard<'g>
		= MutexRef<'g, T, R>
	where
		Self: 'g;
	type ReadGuard<'g>
		= MutexRef<'g, T, R>
	where
		Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		ptrs.push(self);
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		MutexRef::new(self)
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		MutexRef::new(self)
	}
}

impl<T: Send, R: RawMutex + Send + Sync> LockableIntoInner for Mutex<T, R> {
	type Inner = T;

	fn into_inner(self) -> Self::Inner {
		self.into_inner()
	}
}

impl<T: Send, R: RawMutex + Send + Sync> LockableAsMut for Mutex<T, R> {
	type Inner<'a>
		= &'a mut T
	where
		Self: 'a;

	fn as_mut(&mut self) -> Self::Inner<'_> {
		self.get_mut()
	}
}

unsafe impl<T: Send, R: RawMutex + Send + Sync> OwnedLockable for Mutex<T, R> {}

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
	pub fn lock<'s, 'k: 's, Key: Keyable>(&'s self, key: Key) -> MutexGuard<'s, 'k, T, Key, R> {
		unsafe {
			// safety: we have the thread key
			self.raw_lock();

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
	) -> Option<MutexGuard<'s, 'k, T, Key, R>> {
		unsafe {
			// safety: we have the key to the mutex
			self.raw_try_lock().then(||
				// safety: we just locked the mutex
				MutexGuard::new(self, key))
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
		self.raw_try_lock().then_some(MutexRef(self, PhantomData))
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
			guard.mutex.0.raw_unlock();
		}
		guard.thread_key
	}
}

unsafe impl<R: RawMutex + Send, T: ?Sized + Send> Send for Mutex<T, R> {}
unsafe impl<R: RawMutex + Sync, T: ?Sized + Send> Sync for Mutex<T, R> {}
