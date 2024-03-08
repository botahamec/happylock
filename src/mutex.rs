use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};

use crate::lock::Lock;
use crate::ThreadKey;

/// A spinning mutex
pub type SpinLock<T> = Mutex<T, RawSpin>;

/// Implements a raw C-like mutex.
///
/// # Safety
///
/// It cannot be possible to lock the mutex when it is already locked.
pub unsafe trait RawMutex {
	/// The initial value for an unlocked mutex
	const INIT: Self;

	/// Lock the mutex, blocking until the lock is acquired
	fn lock(&self);

	/// Attempt to lock the mutex without blocking.
	///
	/// Returns `true` if successful, `false` otherwise.
	fn try_lock(&self) -> bool;

	/// Checks whether the mutex is currently locked or not
	fn is_locked(&self) -> bool;

	/// Unlock the mutex.
	///
	/// # Safety
	///
	/// The lock must be acquired in the current context.
	unsafe fn unlock(&self);
}

/// A raw mutex which just spins
pub struct RawSpin {
	lock: Lock,
}

unsafe impl RawMutex for RawSpin {
	const INIT: Self = Self { lock: Lock::new() };

	fn lock(&self) {
		loop {
			std::hint::spin_loop();

			if let Some(key) = self.lock.try_lock() {
				std::mem::forget(key);
				return;
			}
		}
	}

	fn try_lock(&self) -> bool {
		self.lock.try_lock().is_some()
	}

	fn is_locked(&self) -> bool {
		self.lock.is_locked()
	}

	unsafe fn unlock(&self) {
		self.lock.force_unlock();
	}
}

/// A mutual exclusion primitive useful for protecting shared data, which
/// cannot deadlock.
///
/// This mutex will block threads waiting for the lock to become available.
/// Each mutex has a type parameter which represents the data that it is
/// protecting. The data can only be accessed through the [`MutexGuard`]s
/// returned from [`lock`] and [`try_lock`], which guarantees that the data is
/// only ever accessed when the mutex is locked.
///
/// Locking the mutex on a thread that already locked it is impossible, due to
/// the requirement of the [`ThreadKey`]. Therefore, this will never deadlock.
///
/// [`lock`]: `Mutex::lock`
/// [`try_lock`]: `Mutex::try_lock`
pub struct Mutex<T: ?Sized, R> {
	raw: R,
	value: UnsafeCell<T>,
}

/// A reference to a mutex that unlocks it when dropped
pub struct MutexRef<'a, T: ?Sized + 'a, R: RawMutex>(&'a Mutex<T, R>);

impl<'a, T: ?Sized + 'a, R: RawMutex> Drop for MutexRef<'a, T, R> {
	fn drop(&mut self) {
		// safety: this guard is being destroyed, so the data cannot be
		//         accessed without locking again
		unsafe { self.0.force_unlock() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawMutex> Deref for MutexRef<'a, T, R> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// safety: this is the only type that can use `value`, and there's
		//         a reference to this type, so there cannot be any mutable
		//         references to this value.
		unsafe { &*self.0.value.get() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawMutex> DerefMut for MutexRef<'a, T, R> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		// safety: this is the only type that can use `value`, and we have a
		//         mutable reference to this type, so there cannot be any other
		//         references to this value.
		unsafe { &mut *self.0.value.get() }
	}
}

/// An RAII implementation of a “scoped lock” of a mutex. When this structure
/// is dropped (falls out of scope), the lock will be unlocked.
///
/// This is created by calling the [`lock`] and [`try_lock`] methods on [`Mutex`]
///
/// [`lock`]: `Mutex::lock`
/// [`try_lock`]: `Mutex::try_lock`
pub struct MutexGuard<'a, T: ?Sized + 'a, R: RawMutex = RawSpin> {
	mutex: MutexRef<'a, T, R>,
	_thread_key: &'a mut ThreadKey,
}

impl<'a, T: ?Sized + 'a, R: RawMutex> Deref for MutexGuard<'a, T, R> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.mutex
	}
}

impl<'a, T: ?Sized + 'a, R: RawMutex> DerefMut for MutexGuard<'a, T, R> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.mutex
	}
}

impl<'a, T: ?Sized + 'a, R: RawMutex> MutexGuard<'a, T, R> {
	/// Create a guard to the given mutex. Undefined if multiple guards to the
	/// same mutex exist at once.
	unsafe fn new(mutex: &'a Mutex<T, R>, thread_key: &'a mut ThreadKey) -> Self {
		Self {
			mutex: MutexRef(mutex),
			_thread_key: thread_key,
		}
	}
}

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
	pub const fn new(value: T) -> Self {
		Self {
			raw: R::INIT,
			value: UnsafeCell::new(value),
		}
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
	pub fn lock<'s, 'a: 's>(&'s self, key: &'a mut ThreadKey) -> MutexGuard<'_, T, R> {
		self.raw.lock();

		// safety: we just locked the mutex
		unsafe { MutexGuard::new(self, key) }
	}

	/// Lock without a [`ThreadKey`]. You must mutually own the [`ThreadKey`] as
	/// long as the [`MutexRef`] is alive. This may cause deadlock if called
	/// multiple times without unlocking first.
	pub(crate) unsafe fn lock_ref(&self) -> MutexRef<'_, T, R> {
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
	/// let c_mutex = Arc::clone(mutex);
	///
	/// thread::spawn(move || {
	///     let key = ThradKey::lock().unwrap();
	///     let mut lock = c_mutex.try_lock();
	///     if let Ok(lock) = lock {
	///         **mutex = 10;
	///     } else {
	///         println!("try_lock failed");
	///     }
	/// }).join().expect("thread::spawn failed");
	///
	/// let key = ThreadKey::lock().unwrap();
	/// assert_eq!(*mutex.lock(key), 10);
	/// ```
	pub fn try_lock<'s, 'a: 's>(&'s self, key: &'a mut ThreadKey) -> Option<MutexGuard<'_, T, R>> {
		if self.raw.try_lock() {
			// safety: we just locked the mutex
			Some(unsafe { MutexGuard::new(self, key) })
		} else {
			None
		}
	}

	/// Lock without a [`ThreadKey`]. It is undefined behavior to do this without
	/// owning the [`ThreadKey`].
	pub(crate) unsafe fn try_lock_ref(&self) -> Option<MutexRef<'_, T, R>> {
		self.raw.try_lock().then_some(MutexRef(self))
	}

	/// Forcibly unlocks the `Lock`.
	///
	/// # Safety
	///
	/// This should only be called if there are no references to any
	/// [`MutexGuard`]s for this mutex in the program.
	unsafe fn force_unlock(&self) {
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
	/// let guard = mutex.lock(key);
	/// *guard += 20;
	///
	/// let key = Mutex::unlock(guard);
	/// ```
	#[allow(clippy::missing_const_for_fn)]
	pub fn unlock(guard: MutexGuard<'_, T, R>) {
		drop(guard);
	}
}

unsafe impl<R: Send, T: ?Sized + Send> Send for Mutex<T, R> {}
unsafe impl<R: RawMutex + Sync, T: ?Sized + Send> Sync for Mutex<T, R> {}
