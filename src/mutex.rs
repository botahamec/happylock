use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use lock_api::RawMutex;

use crate::key::Keyable;

/// A spinning mutex
pub type SpinLock<T> = Mutex<T, spin::Mutex<()>>;

/// A parking lot mutex
pub type ParkingMutex<T> = Mutex<T, parking_lot::RawMutex>;

/// A standard library mutex
pub type StdMutex<T> = Mutex<T, antidote::Mutex<()>>;

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
pub struct Mutex<T: ?Sized, R: RawMutex> {
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
pub struct MutexGuard<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable + 'key, R: RawMutex> {
	mutex: MutexRef<'a, T, R>,
	thread_key: Key,
	_phantom: PhantomData<&'key ()>,
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawMutex> Deref
	for MutexGuard<'a, 'key, T, Key, R>
{
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.mutex
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawMutex> DerefMut
	for MutexGuard<'a, 'key, T, Key, R>
{
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.mutex
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawMutex> MutexGuard<'a, 'key, T, Key, R> {
	/// Create a guard to the given mutex. Undefined if multiple guards to the
	/// same mutex exist at once.
	const unsafe fn new(mutex: &'a Mutex<T, R>, thread_key: Key) -> Self {
		Self {
			mutex: MutexRef(mutex),
			thread_key,
			_phantom: PhantomData,
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
	pub fn lock<'s, 'a: 's, 'k: 'a, Key: Keyable>(
		&'s self,
		key: Key,
	) -> MutexGuard<'_, 'k, T, Key, R> {
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
	pub fn unlock<'a, 'k: 'a, Key: Keyable + 'k>(guard: MutexGuard<'a, 'k, T, Key, R>) -> Key {
		unsafe {
			guard.mutex.0.force_unlock();
		}
		guard.thread_key
	}
}

unsafe impl<R: RawMutex + Send, T: ?Sized + Send> Send for Mutex<T, R> {}
unsafe impl<R: RawMutex + Sync, T: ?Sized + Send> Sync for Mutex<T, R> {}
