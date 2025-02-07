use std::marker::PhantomData;
use std::sync::atomic::AtomicBool;

mod error;
mod flag;
mod guard;
mod poisonable;

/// A flag indicating if a lock is poisoned or not. The implementation differs
/// depending on whether panics are set to unwind or abort.
#[derive(Debug, Default)]
pub(crate) struct PoisonFlag(#[cfg(panic = "unwind")] AtomicBool);

/// A wrapper around [`Lockable`] types which will enable poisoning.
///
/// A lock is "poisoned" when the thread panics while holding the lock. Once a
/// lock is poisoned, all other threads are unable to access the data by
/// default, because the data may be tainted (some invariant of the data might
/// not be upheld).
///
/// The [`lock`] and [`try_lock`] methods return a [`Result`] which indicates
/// whether the lock has been poisoned or not. The [`PoisonError`] type has an
/// [`into_inner`] method which will return the guard that normally would have
/// been returned for a successful lock. This allows access to the data,
/// despite the lock being poisoned.
///
/// Alternatively, there is also a [`clear_poison`] method, which should
/// indicate that all invariants of the underlying data are upheld, so that
/// subsequent calls may still return [`Ok`].
///
/// [`Lockable`]: `crate::lockable::Lockable`
/// [`lock`]: `Poisonable::lock`
/// [`try_lock`]: `Poisonable::try_lock`
/// [`into_inner`]: `PoisonError::into_inner`
/// [`clear_poison`]: `Poisonable::clear_poison`
#[derive(Debug, Default)]
pub struct Poisonable<L> {
	inner: L,
	poisoned: PoisonFlag,
}

/// An RAII guard for a [`Poisonable`].
///
/// This is similar to a [`PoisonGuard`], except that it does not hold a
/// [`Keyable`]
///
/// [`Keyable`]: `crate::Keyable`
pub struct PoisonRef<'a, G> {
	guard: G,
	#[cfg(panic = "unwind")]
	flag: &'a PoisonFlag,
	_phantom: PhantomData<&'a ()>,
}

/// An RAII guard for a [`Poisonable`].
///
/// This is created by calling methods like [`Poisonable::lock`].
pub struct PoisonGuard<'a, 'key: 'a, G, Key: 'key> {
	guard: PoisonRef<'a, G>,
	key: Key,
	_phantom: PhantomData<&'key ()>,
}

/// A type of error which can be returned when acquiring a [`Poisonable`] lock.
pub struct PoisonError<Guard> {
	guard: Guard,
}

/// An enumeration of possible errors associated with
/// [`TryLockPoisonableResult`] which can occur while trying to acquire a lock
/// (i.e.: [`Poisonable::try_lock`]).
pub enum TryLockPoisonableError<'flag, 'key: 'flag, G, Key: 'key> {
	Poisoned(PoisonError<PoisonGuard<'flag, 'key, G, Key>>),
	WouldBlock(Key),
}

/// A type alias for the result of a lock method which can poisoned.
///
/// The [`Ok`] variant of this result indicates that the primitive was not
/// poisoned, and the primitive was poisoned. Note that the [`Err`] variant
/// *also* carries the associated guard, and it can be acquired through the
/// [`into_inner`] method.
///
/// [`into_inner`]: `PoisonError::into_inner`
pub type PoisonResult<Guard> = Result<Guard, PoisonError<Guard>>;

/// A type alias for the result of a nonblocking locking method.
///
/// For more information, see [`PoisonResult`]. A `TryLockPoisonableResult`
/// doesn't necessarily hold the associated guard in the [`Err`] type as the
/// lock might not have been acquired for other reasons.
pub type TryLockPoisonableResult<'flag, 'key, G, Key> =
	Result<PoisonGuard<'flag, 'key, G, Key>, TryLockPoisonableError<'flag, 'key, G, Key>>;

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use super::*;
	use crate::lockable::Lockable;
	use crate::{LockCollection, Mutex, ThreadKey};

	#[test]
	fn display_works() {
		let key = ThreadKey::get().unwrap();
		let mutex = Poisonable::new(Mutex::new("Hello, world!"));

		let guard = mutex.lock(key).unwrap();

		assert_eq!(guard.to_string(), "Hello, world!");
	}

	#[test]
	fn ord_works() {
		let key = ThreadKey::get().unwrap();
		let lock1 = Poisonable::new(Mutex::new(1));
		let lock2 = Poisonable::new(Mutex::new(3));
		let lock3 = Poisonable::new(Mutex::new(3));
		let collection = LockCollection::try_new((&lock1, &lock2, &lock3)).unwrap();

		let guard = collection.lock(key);
		let guard1 = guard.0.as_ref().unwrap();
		let guard2 = guard.1.as_ref().unwrap();
		let guard3 = guard.2.as_ref().unwrap();
		assert_eq!(guard1.cmp(guard2), std::cmp::Ordering::Less);
		assert_eq!(guard2.cmp(guard1), std::cmp::Ordering::Greater);
		assert!(guard2 == guard3);
		assert!(guard1 != guard3);
	}

	#[test]
	fn get_ptrs() {
		let mutex = Mutex::new(5);
		let poisonable = Poisonable::new(mutex);
		let mut lock_ptrs = Vec::new();
		poisonable.get_ptrs(&mut lock_ptrs);

		assert_eq!(lock_ptrs.len(), 1);
		assert!(std::ptr::addr_eq(lock_ptrs[0], &poisonable.inner));
	}

	#[test]
	fn clear_poison_for_poisoned_mutex() {
		let mutex = Arc::new(Poisonable::new(Mutex::new(0)));
		let c_mutex = Arc::clone(&mutex);

		let _ = std::thread::spawn(move || {
			let key = ThreadKey::get().unwrap();
			let _lock = c_mutex.lock(key).unwrap();
			panic!(); // the mutex gets poisoned
		})
		.join();

		assert!(mutex.is_poisoned());

		let key = ThreadKey::get().unwrap();
		let _ = mutex.lock(key).unwrap_or_else(|mut e| {
			**e.get_mut() = 1;
			mutex.clear_poison();
			e.into_inner()
		});

		assert!(!mutex.is_poisoned());
	}

	#[test]
	fn error_as_ref() {
		let mutex = Poisonable::new(Mutex::new("foo"));

		let _ = std::panic::catch_unwind(|| {
			let key = ThreadKey::get().unwrap();
			#[allow(unused_variables)]
			let guard = mutex.lock(key);
			panic!();

			#[allow(unknown_lints)]
			#[allow(unreachable_code)]
			drop(guard);
		});

		assert!(mutex.is_poisoned());

		let key = ThreadKey::get().unwrap();
		let error = mutex.lock(key).unwrap_err();
		assert_eq!(&***error.as_ref(), "foo");
	}

	#[test]
	fn error_as_mut() {
		let mutex = Poisonable::new(Mutex::new("foo"));

		let _ = std::panic::catch_unwind(|| {
			let key = ThreadKey::get().unwrap();
			#[allow(unused_variables)]
			let guard = mutex.lock(key);
			panic!();

			#[allow(unknown_lints)]
			#[allow(unreachable_code)]
			drop(guard);
		});

		assert!(mutex.is_poisoned());

		let mut key = ThreadKey::get().unwrap();
		let mut error = mutex.lock(&mut key).unwrap_err();
		let error1 = error.as_mut();
		**error1 = "bar";
		drop(error);

		mutex.clear_poison();
		let guard = mutex.lock(&mut key).unwrap();
		assert_eq!(&**guard, "bar");
	}

	#[test]
	fn new_poisonable_is_not_poisoned() {
		let mutex = Poisonable::new(Mutex::new(42));
		assert!(!mutex.is_poisoned());
	}
}
