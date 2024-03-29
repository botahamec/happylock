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
/// [`lock`]: `Mutex::lock`
/// [`try_lock`]: `Mutex::try_lock`
/// [`ThreadKey`]: `crate::ThreadKey`
pub struct Mutex<T: ?Sized, R> {
	raw: R,
	data: UnsafeCell<T>,
}

/// A reference to a mutex that unlocks it when dropped
pub struct MutexRef<'a, T: ?Sized + 'a, R: RawMutex>(
	&'a Mutex<T, R>,
	PhantomData<(&'a mut T, R::GuardMarker)>,
);

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

struct MutexLockFuture<'a, T: ?Sized + 'a, R: RawMutex> {
	mutex: &'a Mutex<T, R>,
	key: Option<crate::ThreadKey>,
}

impl<'a, T: ?Sized + 'a, R: RawMutex> std::future::Future for MutexLockFuture<'a, T, R> {
	type Output = MutexGuard<'a, 'a, T, crate::ThreadKey, R>;

	fn poll(
		mut self: std::pin::Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
	) -> std::task::Poll<Self::Output> {
		match unsafe { self.mutex.try_lock_no_key() } {
			Some(guard) => std::task::Poll::Ready(unsafe {
				MutexGuard::new(guard.0, self.key.take().unwrap())
			}),
			None => {
				cx.waker().wake_by_ref();
				std::task::Poll::Pending
			}
		}
	}
}
