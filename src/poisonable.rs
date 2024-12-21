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
pub struct PoisonGuard<'a, 'key, G, Key> {
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
pub enum TryLockPoisonableError<'flag, 'key, G, Key: 'key> {
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
	use super::*;
	use crate::lockable::Lockable;
	use crate::{Mutex, ThreadKey};

	#[test]
	fn display_works() {
		let key = ThreadKey::get().unwrap();
		let mutex = Poisonable::new(Mutex::new("Hello, world!"));

		let guard = mutex.lock(key).unwrap();

		assert_eq!(guard.to_string(), "Hello, world!");
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
}
