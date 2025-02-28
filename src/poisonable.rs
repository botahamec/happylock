use std::marker::PhantomData;
use std::sync::atomic::AtomicBool;

use crate::ThreadKey;

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
pub struct PoisonGuard<'a, G> {
	guard: PoisonRef<'a, G>,
	key: ThreadKey,
}

/// A type of error which can be returned when acquiring a [`Poisonable`] lock.
pub struct PoisonError<Guard> {
	guard: Guard,
}

/// An enumeration of possible errors associated with
/// [`TryLockPoisonableResult`] which can occur while trying to acquire a lock
/// (i.e.: [`Poisonable::try_lock`]).
pub enum TryLockPoisonableError<'flag, G> {
	Poisoned(PoisonError<PoisonGuard<'flag, G>>),
	WouldBlock(ThreadKey),
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
pub type TryLockPoisonableResult<'flag, G> =
	Result<PoisonGuard<'flag, G>, TryLockPoisonableError<'flag, G>>;

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use super::*;
	use crate::lockable::Lockable;
	use crate::{LockCollection, Mutex, ThreadKey};

	#[test]
	fn locking_poisoned_mutex_returns_error_in_collection() {
		let key = ThreadKey::get().unwrap();
		let mutex = LockCollection::new(Poisonable::new(Mutex::new(42)));

		std::thread::scope(|s| {
			s.spawn(|| {
				let key = ThreadKey::get().unwrap();
				let mut guard1 = mutex.lock(key);
				let guard = guard1.as_deref_mut().unwrap();
				assert_eq!(**guard, 42);
				panic!();

				#[allow(unreachable_code)]
				drop(guard1);
			})
			.join()
			.unwrap_err();
		});

		let error = mutex.lock(key);
		let error = error.as_deref().unwrap_err();
		assert_eq!(***error.get_ref(), 42);
	}

	#[test]
	fn non_poisoned_get_mut_is_ok() {
		let mut mutex = Poisonable::new(Mutex::new(42));
		let guard = mutex.get_mut();
		assert!(guard.is_ok());
		assert_eq!(*guard.unwrap(), 42);
	}

	#[test]
	fn non_poisoned_get_mut_is_err() {
		let mut mutex = Poisonable::new(Mutex::new(42));

		let _ = std::panic::catch_unwind(|| {
			let key = ThreadKey::get().unwrap();
			#[allow(unused_variables)]
			let guard = mutex.lock(key);
			panic!();
			#[allow(unreachable_code)]
			drop(guard);
		});

		let guard = mutex.get_mut();
		assert!(guard.is_err());
		assert_eq!(**guard.unwrap_err().get_ref(), 42);
	}

	#[test]
	fn unpoisoned_into_inner() {
		let mutex = Poisonable::new(Mutex::new("foo"));
		assert_eq!(mutex.into_inner().unwrap(), "foo");
	}

	#[test]
	fn poisoned_into_inner() {
		let mutex = Poisonable::from(Mutex::new("foo"));

		std::panic::catch_unwind(|| {
			let key = ThreadKey::get().unwrap();
			#[allow(unused_variables)]
			let guard = mutex.lock(key);
			panic!();
			#[allow(unreachable_code)]
			drop(guard);
		})
		.unwrap_err();

		let error = mutex.into_inner().unwrap_err();
		assert_eq!(error.into_inner(), "foo");
	}

	#[test]
	fn unpoisoned_into_child() {
		let mutex = Poisonable::new(Mutex::new("foo"));
		assert_eq!(mutex.into_child().unwrap().into_inner(), "foo");
	}

	#[test]
	fn poisoned_into_child() {
		let mutex = Poisonable::from(Mutex::new("foo"));

		std::panic::catch_unwind(|| {
			let key = ThreadKey::get().unwrap();
			#[allow(unused_variables)]
			let guard = mutex.lock(key);
			panic!();
			#[allow(unreachable_code)]
			drop(guard);
		})
		.unwrap_err();

		let error = mutex.into_child().unwrap_err();
		assert_eq!(error.into_inner().into_inner(), "foo");
	}

	#[test]
	fn display_works() {
		let key = ThreadKey::get().unwrap();
		let mutex = Poisonable::new(Mutex::new("Hello, world!"));

		let guard = mutex.lock(key).unwrap();

		assert_eq!(guard.to_string(), "Hello, world!");
	}

	#[test]
	fn ref_as_ref() {
		let key = ThreadKey::get().unwrap();
		let collection = LockCollection::new(Poisonable::new(Mutex::new("foo")));
		let guard = collection.lock(key);
		let Ok(ref guard) = guard.as_ref() else {
			panic!()
		};
		assert_eq!(**guard.as_ref(), "foo");
	}

	#[test]
	fn ref_as_mut() {
		let key = ThreadKey::get().unwrap();
		let collection = LockCollection::new(Poisonable::new(Mutex::new("foo")));
		let mut guard1 = collection.lock(key);
		let Ok(ref mut guard) = guard1.as_mut() else {
			panic!()
		};
		let guard = guard.as_mut();
		**guard = "bar";

		let key = LockCollection::<Poisonable<Mutex<_>>>::unlock(guard1);
		let guard = collection.lock(key);
		let guard = guard.as_deref().unwrap();
		assert_eq!(*guard.as_ref(), "bar");
	}

	#[test]
	fn guard_as_ref() {
		let key = ThreadKey::get().unwrap();
		let collection = Poisonable::new(Mutex::new("foo"));
		let guard = collection.lock(key);
		let Ok(ref guard) = guard.as_ref() else {
			panic!()
		};
		assert_eq!(**guard.as_ref(), "foo");
	}

	#[test]
	fn guard_as_mut() {
		let key = ThreadKey::get().unwrap();
		let mutex = Poisonable::new(Mutex::new("foo"));
		let mut guard1 = mutex.lock(key);
		let Ok(ref mut guard) = guard1.as_mut() else {
			panic!()
		};
		let guard = guard.as_mut();
		**guard = "bar";

		let key = Poisonable::<Mutex<_>>::unlock(guard1.unwrap());
		let guard = mutex.lock(key);
		let guard = guard.as_deref().unwrap();
		assert_eq!(*guard, "bar");
	}

	#[test]
	fn deref_mut_in_collection() {
		let key = ThreadKey::get().unwrap();
		let collection = LockCollection::new(Poisonable::new(Mutex::new(42)));
		let mut guard1 = collection.lock(key);
		let Ok(ref mut guard) = guard1.as_mut() else {
			panic!()
		};
		// TODO make this more convenient
		assert_eq!(***guard, 42);
		***guard = 24;

		let key = LockCollection::<Poisonable<Mutex<_>>>::unlock(guard1);
		_ = collection.lock(key);
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

		let key: ThreadKey = ThreadKey::get().unwrap();
		let mut error = mutex.lock(key).unwrap_err();
		let error1 = error.as_mut();
		**error1 = "bar";
		let key = Poisonable::<Mutex<_>>::unlock(error.into_inner());

		mutex.clear_poison();
		let guard = mutex.lock(key).unwrap();
		assert_eq!(&**guard, "bar");
	}

	#[test]
	fn try_error_from_lock_error() {
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
		let error = TryLockPoisonableError::from(error);

		let TryLockPoisonableError::Poisoned(error) = error else {
			panic!()
		};
		assert_eq!(&**error.into_inner(), "foo");
	}

	#[test]
	fn new_poisonable_is_not_poisoned() {
		let mutex = Poisonable::new(Mutex::new(42));
		assert!(!mutex.is_poisoned());
	}
}
