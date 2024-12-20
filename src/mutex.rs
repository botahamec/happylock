use std::cell::UnsafeCell;
use std::marker::PhantomData;

use lock_api::RawMutex;

use crate::key::Keyable;

mod guard;
mod mutex;

/// A spinning mutex
#[cfg(feature = "spin")]
pub type SpinLock<T> = Mutex<T, spin::Mutex<()>>;

/// A parking lot mutex
#[cfg(feature = "parking_lot")]
pub type ParkingMutex<T> = Mutex<T, parking_lot::RawMutex>;

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
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use std::thread;
/// use std::sync::mpsc;
///
/// use happylock::{Mutex, ThreadKey};
///
/// // Spawn a few threads to increment a shared variable (non-atomically),
/// // and let the main thread know once all increments are done.
/// //
/// // Here we're using an Arc to share memory among threads, and the data
/// // inside the Arc is protected with a mutex.
/// const N: usize = 10;
///
/// let data = Arc::new(Mutex::new(0));
///
/// let (tx, rx) = mpsc::channel();
/// for _ in 0..N {
///     let (data, tx) = (Arc::clone(&data), tx.clone());
///     thread::spawn(move || {
///         let key = ThreadKey::get().unwrap();
///         let mut data = data.lock(key);
///         *data += 1;
///         if *data == N {
///             tx.send(()).unwrap();
///         }
///         // the lock is unlocked
///     });
/// }
///
/// rx.recv().unwrap();
/// ```
///
/// To unlock a mutex guard sooner than the end of the enclosing scope, either
/// create an inner scope, drop the guard manually, or call [`Mutex::unlock`].
///
/// ```
/// use std::sync::Arc;
/// use std::thread;
///
/// use happylock::{Mutex, ThreadKey};
///
/// const N: usize = 3;
///
/// let data_mutex = Arc::new(Mutex::new(vec![1, 2, 3, 4]));
/// let res_mutex = Arc::new(Mutex::new(0));
///
/// let mut threads = Vec::with_capacity(N);
/// (0..N).for_each(|_| {
///     let data_mutex_clone = Arc::clone(&data_mutex);
///     let res_mutex_clone = Arc::clone(&res_mutex);
///
///     threads.push(thread::spawn(move || {
///         let mut key = ThreadKey::get().unwrap();
///
///         // Here we use a block to limit the lifetime of the lock guard.
///         let result = {
///             let mut data = data_mutex_clone.lock(&mut key);
///             let result = data.iter().fold(0, |acc, x| acc + x * 2);
///             data.push(result);
///             result
///             // The mutex guard gets dropped here, so the lock is released
///         };
///         // The thread key is available again
///         *res_mutex_clone.lock(key) += result;
///     }));
/// });
///
/// let mut key = ThreadKey::get().unwrap();
/// let mut data = data_mutex.lock(&mut key);
/// let result = data.iter().fold(0, |acc, x| acc + x * 2);
/// data.push(result);
///
/// // We drop the `data` explicitly because it's not necessary anymore. This
/// // allows other threads to start working on the data immediately. Dropping
/// // the data also gives us access to the thread key, so we can lock
/// // another mutex.
/// drop(data);
///
/// // Here the mutex guard is not assigned to a variable and so, even if the
/// // scope does not end after this line, the mutex is still released: there is
/// // no deadlock.
/// *res_mutex.lock(&mut key) += result;
///
/// threads.into_iter().for_each(|thread| {
///     thread
///         .join()
///         .expect("The thread creating or execution failed !")
/// });
///
/// assert_eq!(*res_mutex.lock(key), 800);
/// ```
///
/// [`lock`]: `Mutex::lock`
/// [`try_lock`]: `Mutex::try_lock`
/// [`ThreadKey`]: `crate::ThreadKey`
pub struct Mutex<T: ?Sized, R> {
	raw: R,
	data: UnsafeCell<T>,
}

/// A reference to a mutex that unlocks it when dropped.
///
/// This is similar to [`MutexGuard`], except it does not hold a [`Keyable`].
pub struct MutexRef<'a, T: ?Sized + 'a, R: RawMutex>(
	&'a Mutex<T, R>,
	PhantomData<(&'a mut T, R::GuardMarker)>,
);

/// An RAII implementation of a “scoped lock” of a mutex.
///
/// When this structure is dropped (falls out of scope), the lock will be
/// unlocked.
///
/// This is created by calling the [`lock`] and [`try_lock`] methods on [`Mutex`]
///
/// [`lock`]: `Mutex::lock`
/// [`try_lock`]: `Mutex::try_lock`
//
// This is the most lifetime-intensive thing I've ever written. Can I graduate
// from borrow checker university now?
pub struct MutexGuard<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable + 'key, R: RawMutex> {
	mutex: MutexRef<'a, T, R>, // this way we don't need to re-implement Drop
	thread_key: Key,
	_phantom: PhantomData<&'key ()>,
}

#[cfg(test)]
mod tests {
	use crate::ThreadKey;

	use super::*;

	#[test]
	fn unlocked_when_initialized() {
		let lock: crate::Mutex<_> = Mutex::new("Hello, world!");

		assert!(!lock.is_locked());
	}

	#[test]
	fn locked_after_read() {
		let key = ThreadKey::get().unwrap();
		let lock: crate::Mutex<_> = Mutex::new("Hello, world!");

		let guard = lock.lock(key);

		assert!(lock.is_locked());
		drop(guard)
	}

	#[test]
	fn display_works_for_guard() {
		let key = ThreadKey::get().unwrap();
		let mutex: crate::Mutex<_> = Mutex::new("Hello, world!");
		let guard = mutex.lock(key);
		assert_eq!(guard.to_string(), "Hello, world!".to_string());
	}

	#[test]
	fn display_works_for_ref() {
		let mutex: crate::Mutex<_> = Mutex::new("Hello, world!");
		let guard = unsafe { mutex.try_lock_no_key().unwrap() }; // TODO lock_no_key
		assert_eq!(guard.to_string(), "Hello, world!".to_string());
	}

	#[test]
	fn dropping_guard_releases_mutex() {
		let mut key = ThreadKey::get().unwrap();
		let mutex: crate::Mutex<_> = Mutex::new("Hello, world!");

		let guard = mutex.lock(&mut key);
		drop(guard);

		assert!(!mutex.is_locked());
	}

	#[test]
	fn dropping_ref_releases_mutex() {
		let mutex: crate::Mutex<_> = Mutex::new("Hello, world!");

		let guard = unsafe { mutex.try_lock_no_key().unwrap() };
		drop(guard);

		assert!(!mutex.is_locked());
	}
}
