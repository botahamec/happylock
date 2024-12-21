use std::cell::UnsafeCell;
use std::marker::PhantomData;

use lock_api::RawRwLock;

use crate::key::Keyable;
use crate::poisonable::PoisonFlag;

mod rwlock;

mod read_lock;
mod write_lock;

mod read_guard;
mod write_guard;

#[cfg(feature = "spin")]
pub type SpinRwLock<T> = RwLock<T, spin::RwLock<()>>;

#[cfg(feature = "parking_lot")]
pub type ParkingRwLock<T> = RwLock<T, parking_lot::RawRwLock>;

/// A reader-writer lock
///
/// This type of lock allows a number of readers or at most one writer at any
/// point in time. The write portion of this lock typically allows modification
/// of the underlying data (exclusive access) and the read portion of this lock
/// typically allows for read-only access (shared access).
///
/// In comparison, a [`Mutex`] does not distinguish between readers or writers
/// that acquire the lock, therefore blocking any threads waiting for the lock
/// to become available. An `RwLock` will allow any number of readers to
/// acquire the lock as long as a writer is not holding the lock.
///
/// The type parameter T represents the data that this lock protects. It is
/// required that T satisfies [`Send`] to be shared across threads and [`Sync`]
/// to allow concurrent access through readers. The RAII guard returned from
/// the locking methods implement [`Deref`] (and [`DerefMut`] for the `write`
/// methods) to allow access to the content of the lock.
///
/// Locking the mutex on a thread that already locked it is impossible, due to
/// the requirement of the [`ThreadKey`]. Therefore, this will never deadlock.
///
/// [`ThreadKey`]: `crate::ThreadKey`
/// [`Mutex`]: `crate::mutex::Mutex`
/// [`Deref`]: `std::ops::Deref`
/// [`DerefMut`]: `std::ops::DerefMut`
pub struct RwLock<T: ?Sized, R> {
	raw: R,
	poison: PoisonFlag,
	data: UnsafeCell<T>,
}

/// Grants read access to an [`RwLock`]
///
/// This structure is designed to be used in a [`LockCollection`] to indicate
/// that only read access is needed to the data.
///
/// [`LockCollection`]: `crate::LockCollection`
#[repr(transparent)]
pub struct ReadLock<'l, T: ?Sized, R>(&'l RwLock<T, R>);

/// Grants write access to an [`RwLock`]
///
/// This structure is designed to be used in a [`LockCollection`] to indicate
/// that write access is needed to the data.
///
/// [`LockCollection`]: `crate::LockCollection`
#[repr(transparent)]
pub struct WriteLock<'l, T: ?Sized, R>(&'l RwLock<T, R>);

/// RAII structure that unlocks the shared read access to a [`RwLock`]
///
/// This is similar to [`RwLockReadRef`], except it does not hold a
/// [`Keyable`].
pub struct RwLockReadRef<'a, T: ?Sized, R: RawRwLock>(
	&'a RwLock<T, R>,
	PhantomData<(&'a mut T, R::GuardMarker)>,
);

/// RAII structure that unlocks the exclusive write access to a [`RwLock`]
///
/// This is similar to [`RwLockWriteRef`], except it does not hold a
/// [`Keyable`].
pub struct RwLockWriteRef<'a, T: ?Sized, R: RawRwLock>(
	&'a RwLock<T, R>,
	PhantomData<(&'a mut T, R::GuardMarker)>,
);

/// RAII structure used to release the shared read access of a lock when
/// dropped.
///
/// This structure is created by the [`read`] and [`try_read`] methods on
/// [`RwLock`].
///
/// [`read`]: `RwLock::read`
/// [`try_read`]: `RwLock::try_read`
pub struct RwLockReadGuard<'a, 'key, T: ?Sized, Key: Keyable + 'key, R: RawRwLock> {
	rwlock: RwLockReadRef<'a, T, R>,
	thread_key: Key,
	_phantom: PhantomData<&'key ()>,
}

/// RAII structure used to release the exclusive write access of a lock when
/// dropped.
///
/// This structure is created by the [`write`] and [`try_write`] methods on
/// [`RwLock`]
///
/// [`try_write`]: `RwLock::try_write`
pub struct RwLockWriteGuard<'a, 'key, T: ?Sized, Key: Keyable + 'key, R: RawRwLock> {
	rwlock: RwLockWriteRef<'a, T, R>,
	thread_key: Key,
	_phantom: PhantomData<&'key ()>,
}

#[cfg(test)]
mod tests {
	use crate::ThreadKey;

	use super::*;

	#[test]
	fn unlocked_when_initialized() {
		let key = ThreadKey::get().unwrap();
		let lock: crate::RwLock<_> = RwLock::new("Hello, world!");

		assert!(!lock.is_locked());
		assert!(lock.try_write(key).is_some());
	}

	#[test]
	fn read_lock_unlocked_when_initialized() {
		let key = ThreadKey::get().unwrap();
		let lock: crate::RwLock<_> = RwLock::new("Hello, world!");
		let reader = ReadLock::new(&lock);

		assert!(reader.try_lock(key).is_some());
	}

	#[test]
	fn write_lock_unlocked_when_initialized() {
		let key = ThreadKey::get().unwrap();
		let lock: crate::RwLock<_> = RwLock::new("Hello, world!");
		let writer = WriteLock::new(&lock);

		assert!(writer.try_lock(key).is_some());
	}

	#[test]
	fn locked_after_read() {
		let key = ThreadKey::get().unwrap();
		let lock: crate::RwLock<_> = RwLock::new("Hello, world!");

		let guard = lock.read(key);

		assert!(lock.is_locked());
		drop(guard)
	}

	#[test]
	fn locked_after_using_read_lock() {
		let key = ThreadKey::get().unwrap();
		let lock: crate::RwLock<_> = RwLock::new("Hello, world!");
		let reader = ReadLock::new(&lock);

		let guard = reader.lock(key);

		assert!(lock.is_locked());
		drop(guard)
	}

	#[test]
	fn locked_after_write() {
		let key = ThreadKey::get().unwrap();
		let lock: crate::RwLock<_> = RwLock::new("Hello, world!");

		let guard = lock.write(key);

		assert!(lock.is_locked());
		drop(guard)
	}

	#[test]
	fn locked_after_using_write_lock() {
		let key = ThreadKey::get().unwrap();
		let lock: crate::RwLock<_> = RwLock::new("Hello, world!");
		let writer = WriteLock::new(&lock);

		let guard = writer.lock(key);

		assert!(lock.is_locked());
		drop(guard)
	}

	#[test]
	fn read_display_works() {
		let key = ThreadKey::get().unwrap();
		let lock: crate::RwLock<_> = RwLock::new("Hello, world!");
		let guard = lock.read(key);
		assert_eq!(guard.to_string(), "Hello, world!".to_string());
	}

	#[test]
	fn write_display_works() {
		let key = ThreadKey::get().unwrap();
		let lock: crate::RwLock<_> = RwLock::new("Hello, world!");
		let guard = lock.write(key);
		assert_eq!(guard.to_string(), "Hello, world!".to_string());
	}

	#[test]
	fn read_ref_display_works() {
		let lock: crate::RwLock<_> = RwLock::new("Hello, world!");
		let guard = unsafe { lock.try_read_no_key().unwrap() };
		assert_eq!(guard.to_string(), "Hello, world!".to_string());
	}

	#[test]
	fn write_ref_display_works() {
		let lock: crate::RwLock<_> = RwLock::new("Hello, world!");
		let guard = unsafe { lock.try_write_no_key().unwrap() };
		assert_eq!(guard.to_string(), "Hello, world!".to_string());
	}

	#[test]
	fn dropping_read_ref_releases_rwlock() {
		let lock: crate::RwLock<_> = RwLock::new("Hello, world!");

		let guard = unsafe { lock.try_read_no_key().unwrap() };
		drop(guard);

		assert!(!lock.is_locked());
	}

	#[test]
	fn dropping_write_guard_releases_rwlock() {
		let mut key = ThreadKey::get().unwrap();
		let lock: crate::RwLock<_> = RwLock::new("Hello, world!");

		let guard = lock.write(&mut key);
		drop(guard);

		assert!(!lock.is_locked());
	}
}
