use std::cell::UnsafeCell;
use std::marker::PhantomData;

use lock_api::RawRwLock;

use crate::poisonable::PoisonFlag;
use crate::ThreadKey;

mod rwlock;

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
/// the requirement of the [`ThreadKey`]. This will never deadlock.
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

/// RAII structure that unlocks the shared read access to a [`RwLock`] when
/// dropped.
///
/// This structure is created when the [`RwLock`] is put in a wrapper type,
/// such as [`LockCollection`], and a read-only guard is obtained through the
/// wrapper.
///
/// This is similar to [`RwLockReadGuard`], except it does not hold a
/// [`ThreadKey`].
///
/// [`Deref`]: `std::ops::Deref`
/// [`DerefMut`]: `std::ops::DerefMut`
/// [`LockCollection`]: `crate::LockCollection`
pub struct RwLockReadRef<'a, T: ?Sized, R: RawRwLock>(
	&'a RwLock<T, R>,
	PhantomData<R::GuardMarker>,
);

/// RAII structure that unlocks the exclusive write access to a [`RwLock`] when
/// dropped.
///
/// This structure is created when the [`RwLock`] is put in a wrapper type,
/// such as [`LockCollection`], and a mutable guard is obtained through the
/// wrapper.
///
/// This is similar to [`RwLockWriteGuard`], except it does not hold a
/// [`ThreadKey`].
///
/// [`Deref`]: `std::ops::Deref`
/// [`DerefMut`]: `std::ops::DerefMut`
/// [`LockCollection`]: `crate::LockCollection`
pub struct RwLockWriteRef<'a, T: ?Sized, R: RawRwLock>(
	&'a RwLock<T, R>,
	PhantomData<R::GuardMarker>,
);

/// RAII structure used to release the shared read access of a lock when
/// dropped.
///
/// This structure is created by the [`read`] and [`try_read`] methods on
/// [`RwLock`].
///
/// This guard holds a [`ThreadKey`] for its entire lifetime. Therefore, a new
/// lock cannot be acquired until this one is dropped. The [`ThreadKey`] can be
/// reacquired using [`RwLock::unlock_read`].
///
/// [`Deref`]: `std::ops::Deref`
/// [`DerefMut`]: `std::ops::DerefMut`
/// [`read`]: `RwLock::read`
/// [`try_read`]: `RwLock::try_read`
pub struct RwLockReadGuard<'a, T: ?Sized, R: RawRwLock> {
	rwlock: RwLockReadRef<'a, T, R>,
	thread_key: ThreadKey,
}

/// RAII structure used to release the exclusive write access of a lock when
/// dropped.
///
/// This structure is created by the [`write`] and [`try_write`] methods on
/// [`RwLock`]
///
/// This guard holds a [`ThreadKey`] for its entire lifetime. Therefor, a new
/// lock cannot be acquired until this one is dropped. The [`ThreadKey`] can be
/// reacquired using [`RwLock::unlock_write`].
///
/// [`Deref`]: `std::ops::Deref`
/// [`DerefMut`]: `std::ops::DerefMut`
/// [`try_write`]: `RwLock::try_write`
pub struct RwLockWriteGuard<'a, T: ?Sized, R: RawRwLock> {
	rwlock: RwLockWriteRef<'a, T, R>,
	thread_key: ThreadKey,
}

#[cfg(test)]
mod tests {
	use crate::LockCollection;
	use crate::RwLock;
	use crate::ThreadKey;

	#[test]
	fn unlocked_when_initialized() {
		let key = ThreadKey::get().unwrap();
		let lock: crate::RwLock<_> = RwLock::new("Hello, world!");

		assert!(!lock.is_locked());
		assert!(lock.try_write(key).is_ok());
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
	fn locked_after_write() {
		let key = ThreadKey::get().unwrap();
		let lock: crate::RwLock<_> = RwLock::new("Hello, world!");

		let guard = lock.write(key);

		assert!(lock.is_locked());
		drop(guard)
	}

	#[test]
	fn locked_after_scoped_write() {
		let mut key = ThreadKey::get().unwrap();
		let lock = crate::RwLock::new("Hello, world!");

		lock.scoped_write(&mut key, |guard| {
			assert!(lock.is_locked());
			assert_eq!(*guard, "Hello, world!");

			std::thread::scope(|s| {
				s.spawn(|| {
					let key = ThreadKey::get().unwrap();
					assert!(lock.try_read(key).is_err());
				});
			})
		})
	}

	#[test]
	fn get_mut_works() {
		let key = ThreadKey::get().unwrap();
		let mut lock = crate::RwLock::from(42);

		let mut_ref = lock.get_mut();
		*mut_ref = 24;

		lock.scoped_read(key, |guard| assert_eq!(*guard, 24))
	}

	#[test]
	fn try_write_can_fail() {
		let key = ThreadKey::get().unwrap();
		let lock = crate::RwLock::new("Hello");
		let guard = lock.write(key);

		std::thread::scope(|s| {
			s.spawn(|| {
				let key = ThreadKey::get().unwrap();
				let r = lock.try_write(key);
				assert!(r.is_err());
			});
		});

		drop(guard);
	}

	#[test]
	fn try_read_can_fail() {
		let key = ThreadKey::get().unwrap();
		let lock = crate::RwLock::new("Hello");
		let guard = lock.write(key);

		std::thread::scope(|s| {
			s.spawn(|| {
				let key = ThreadKey::get().unwrap();
				let r = lock.try_read(key);
				assert!(r.is_err());
			});
		});

		drop(guard);
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
		let key = ThreadKey::get().unwrap();
		let lock: crate::RwLock<_> = RwLock::new("Hello, world!");

		let guard = lock.write(key);
		drop(guard);

		assert!(!lock.is_locked());
	}

	#[test]
	fn unlock_write() {
		let key = ThreadKey::get().unwrap();
		let lock = crate::RwLock::new("Hello, world");

		let mut guard = lock.write(key);
		*guard = "Goodbye, world!";
		let key = RwLock::unlock_write(guard);

		let guard = lock.read(key);
		assert_eq!(*guard, "Goodbye, world!");
	}

	#[test]
	fn unlock_read() {
		let key = ThreadKey::get().unwrap();
		let lock = crate::RwLock::new("Hello, world");

		let guard = lock.read(key);
		assert_eq!(*guard, "Hello, world");
		let key = RwLock::unlock_read(guard);

		let guard = lock.write(key);
		assert_eq!(*guard, "Hello, world");
	}

	#[test]
	fn read_ref_as_ref() {
		let key = ThreadKey::get().unwrap();
		let lock = LockCollection::new(crate::RwLock::new("hi"));
		let guard = lock.read(key);

		assert_eq!(*(*guard).as_ref(), "hi");
	}

	#[test]
	fn read_guard_as_ref() {
		let key = ThreadKey::get().unwrap();
		let lock = crate::RwLock::new("hi");
		let guard = lock.read(key);

		assert_eq!(*guard.as_ref(), "hi");
	}

	#[test]
	fn write_ref_as_ref() {
		let key = ThreadKey::get().unwrap();
		let lock = LockCollection::new(crate::RwLock::new("hi"));
		let guard = lock.lock(key);

		assert_eq!(*(*guard).as_ref(), "hi");
	}

	#[test]
	fn write_guard_as_ref() {
		let key = ThreadKey::get().unwrap();
		let lock = crate::RwLock::new("hi");
		let guard = lock.write(key);

		assert_eq!(*guard.as_ref(), "hi");
	}

	#[test]
	fn write_guard_as_mut() {
		let key = ThreadKey::get().unwrap();
		let lock = crate::RwLock::new("hi");
		let mut guard = lock.write(key);

		assert_eq!(*guard.as_mut(), "hi");
		*guard.as_mut() = "foo";
		assert_eq!(*guard.as_mut(), "foo");
	}
}
