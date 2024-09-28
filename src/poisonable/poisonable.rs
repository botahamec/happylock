use std::marker::PhantomData;
use std::panic::{RefUnwindSafe, UnwindSafe};

use crate::lockable::{Lockable, LockableAsMut, LockableIntoInner, RawLock};
use crate::Keyable;

use super::{
	PoisonError, PoisonFlag, PoisonGuard, PoisonRef, PoisonResult, Poisonable,
	TryLockPoisonableError, TryLockPoisonableResult,
};

unsafe impl<L: Lockable + RawLock> Lockable for Poisonable<L> {
	type Guard<'g> = PoisonResult<PoisonRef<'g, L::Guard<'g>>> where Self: 'g;
	type ReadGuard<'g> = PoisonResult<PoisonRef<'g, L::ReadGuard<'g>>> where Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		self.inner.get_ptrs(ptrs)
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		let ref_guard = PoisonRef::new(&self.poisoned, self.inner.guard());

		if self.is_poisoned() {
			Ok(ref_guard)
		} else {
			Err(PoisonError::new(ref_guard))
		}
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		let ref_guard = PoisonRef::new(&self.poisoned, self.inner.read_guard());

		if self.is_poisoned() {
			Ok(ref_guard)
		} else {
			Err(PoisonError::new(ref_guard))
		}
	}
}

impl<L: Lockable + RawLock> From<L> for Poisonable<L> {
	fn from(value: L) -> Self {
		Self::new(value)
	}
}

impl<L: Lockable + RawLock> Poisonable<L> {
	/// Creates a new `Poisonable`
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, Poisonable};
	///
	/// let mutex = Poisonable::new(Mutex::new(0));
	/// ```
	pub const fn new(value: L) -> Self {
		Self {
			inner: value,
			poisoned: PoisonFlag::new(),
		}
	}

	unsafe fn guard<'flag, 'key, Key: Keyable + 'key>(
		&'flag self,
		key: Key,
	) -> PoisonResult<PoisonGuard<'flag, 'key, L::Guard<'flag>, Key>> {
		let guard = PoisonGuard {
			guard: PoisonRef::new(&self.poisoned, self.inner.guard()),
			key,
			_phantom: PhantomData,
		};

		if self.is_poisoned() {
			return Err(PoisonError::new(guard));
		}

		Ok(guard)
	}

	/// Acquires the lock, blocking the current thread until it is ok to do so.
	///
	/// This function will block the current thread until it is available to
	/// acquire the mutex. Upon returning, the thread is the only thread with
	/// the lock held. An RAII guard is returned to allow scoped unlock of the
	/// lock. When the guard goes out of scope, the mutex will be unlocked.
	///
	/// # Errors
	///
	/// If another use of this mutex panicked while holding the mutex, then
	/// this call will return an error once thr mutex is acquired.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use std::thread;
	///
	/// use happylock::{Mutex, Poisonable, ThreadKey};
	///
	/// let mutex = Arc::new(Poisonable::new(Mutex::new(0)));
	/// let c_mutex = Arc::clone(&mutex);
	///
	/// thread::spawn(move || {
	///     let key = ThreadKey::get().unwrap();
	///     *c_mutex.lock(key).unwrap() = 10;
	/// }).join().expect("thread::spawn failed");
	///
	/// let key = ThreadKey::get().unwrap();
	/// assert_eq!(*mutex.lock(key).unwrap(), 10);
	/// ```
	pub fn lock<'flag, 'key, Key: Keyable + 'key>(
		&'flag self,
		key: Key,
	) -> PoisonResult<PoisonGuard<'flag, 'key, L::Guard<'flag>, Key>> {
		unsafe {
			self.inner.lock();
			self.guard(key)
		}
	}

	/// Attempts to acquire this lock.
	///
	/// If the lock could not be acquired at this time, then [`Err`] is
	/// returned. Otherwise, an RAII guard is returned. The lock will be
	/// unlocked when the guard is dropped.
	///
	/// This function does not block.
	///
	/// # Errors
	///
	/// If another user of this mutex panicked while holding the mutex, then
	/// this call will return the [`Poisoned`] error if the mutex would
	/// otherwise be acquired.
	///
	/// If the mutex could not be acquired because it is already locked, then
	/// this call will return the [`WouldBlock`] error.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use std::thread;
	///
	/// use happylock::{Mutex, Poisonable, ThreadKey};
	///
	/// let mutex = Arc::new(Poisonable::new(Mutex::new(0)));
	/// let c_mutex = Arc::clone(&mutex);
	///
	/// thread::spawn(move || {
	///     let key = ThreadKey::get().unwrap();
	///     let mut lock = c_mutex.try_lock(key);
	///     if let Ok(mut mutex) = lock {
	///         *mutex = 10;
	///     } else {
	///         println!("try_lock failed");
	///     }
	/// }).join().expect("thread::spawn failed");
	///
	/// let key = ThreadKey::get().unwrap();
	/// assert_eq!(*mutex.lock(key).unwrap(), 10);
	/// ```
	///
	/// [`Poisoned`]: `TryLockPoisonableError::Poisoned`
	/// [`WouldBlock`]: `TryLockPoisonableError::WouldBlock`
	pub fn try_lock<'flag, 'key, Key: Keyable + 'key>(
		&'flag self,
		key: Key,
	) -> TryLockPoisonableResult<'flag, 'key, L::Guard<'flag>, Key> {
		unsafe {
			if self.inner.try_lock() {
				Ok(self.guard(key)?)
			} else {
				Err(TryLockPoisonableError::WouldBlock(key))
			}
		}
	}

	/// Consumes the [`PoisonGuard`], and consequently unlocks its `Poisonable`.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{ThreadKey, Mutex, Poisonable};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let mutex = Poisonable::new(Mutex::new(0));
	///
	/// let mut guard = mutex.lock(key).unwrap();
	/// *guard += 20;
	///
	/// let key = Poisonable::<Mutex<_>>::unlock(guard);
	/// ```
	pub fn unlock<'flag, 'key, Key: Keyable + 'key>(
		guard: PoisonGuard<'flag, 'key, L::Guard<'flag>, Key>,
	) -> Key {
		drop(guard.guard);
		guard.key
	}

	/// Determines whether the mutex is poisoned.
	///
	/// If another thread is active, the mutex can still become poisoned at any
	/// time. You should not trust a `false` value for program correctness
	/// without additional synchronization.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use std::thread;
	///
	/// use happylock::{Mutex, Poisonable, ThreadKey};
	///
	/// let mutex = Arc::new(Poisonable::new(Mutex::new(0)));
	/// let c_mutex = Arc::clone(&mutex);
	///
	/// let _ = thread::spawn(move || {
	///     let key = ThreadKey::get().unwrap();
	///     let _lock = c_mutex.lock(key).unwrap();
	///     panic!(); // the mutex gets poisoned
	/// }).join();
	///
	/// assert_eq!(mutex.is_poisoned(), true);
	/// ```
	pub fn is_poisoned(&self) -> bool {
		self.poisoned.is_poisoned()
	}

	/// Clear the poisoned state from a lock.
	///
	/// If the lock is poisoned, it will remain poisoned until this function
	/// is called. This allows recovering from a poisoned state and marking
	/// that it has recovered. For example, if the value is overwritten by a
	/// known-good value, then the lock can be marked as un-poisoned. Or
	/// possibly, the value could by inspected to determine if it is in a
	/// consistent state, and if so the poison is removed.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use std::thread;
	///
	/// use happylock::{Mutex, Poisonable, ThreadKey};
	///
	/// let mutex = Arc::new(Poisonable::new(Mutex::new(0)));
	/// let c_mutex = Arc::clone(&mutex);
	///
	/// let _ = thread::spawn(move || {
	///     let key = ThreadKey::get().unwrap();
	///     let _lock = c_mutex.lock(key).unwrap();
	///     panic!(); // the mutex gets poisoned
	/// }).join();
	///
	/// assert_eq!(mutex.is_poisoned(), true);
	///
	/// let key = ThreadKey::get().unwrap();
	/// let x = mutex.lock(key).unwrap_or_else(|mut e| {
	///     *e.get_mut() = 1;
	///     mutex.clear_poison();
	///     e.into_inner()
	/// });
	///
	/// assert_eq!(mutex.is_poisoned(), false);
	/// assert_eq!(*x, 1);
	/// ```
	pub fn clear_poison(&self) {
		self.poisoned.clear_poison()
	}

	/// Consumes this `Poisonable`, returning the underlying lock.
	///
	/// # Errors
	///
	/// If another user of this lock panicked while holding the lock, then this
	/// call will return an error instead.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, Poisonable};
	///
	/// let mutex = Poisonable::new(Mutex::new(0));
	/// assert_eq!(mutex.inner_lock().unwrap().into_inner(), 0);
	/// ```
	pub fn into_inner_lock(self) -> PoisonResult<L> {
		if self.is_poisoned() {
			Err(PoisonError::new(self.inner))
		} else {
			Ok(self.inner)
		}
	}

	/// Returns a mutable reference to the underlying lock.
	///
	/// # Errors
	///
	/// If another user of this lock panicked while holding the lock, then
	/// this call will return an error instead.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, Poisonable, ThreadKey};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let mut mutex = Poisonable::new(Mutex::new(0));
	/// *mutex.lock_mut().unwrap().as_mut() = 10;
	/// assert_eq!(*mutex.lock(key).unwrap(), 10);
	/// ```
	pub fn inner_lock_mut(&mut self) -> PoisonResult<&mut L> {
		if self.is_poisoned() {
			Err(PoisonError::new(&mut self.inner))
		} else {
			Ok(&mut self.inner)
		}
	}
}

impl<L: LockableIntoInner + RawLock> Poisonable<L> {
	/// Consumes this `Poisonable`, returning the underlying data.
	///
	/// # Errors
	///
	/// If another user of this lock panicked while holding the lock, then this
	/// call will return an error instead.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, Poisonable};
	///
	/// let mutex = Poisonable::new(Mutex::new(0));
	/// assert_eq!(mutex.into_inner().unwrap(), 0);
	/// ```
	pub fn into_inner(self) -> PoisonResult<L::Inner> {
		if self.is_poisoned() {
			Err(PoisonError::new(self.inner.into_inner()))
		} else {
			Ok(self.inner.into_inner())
		}
	}
}

impl<L: LockableAsMut + RawLock> Poisonable<L> {
	/// Returns a mutable reference to the underlying data.
	///
	/// Since this call borrows the `Poisonable` mutable, no actual locking
	/// needs to take place - the mutable borrow statically guarantees no locks
	/// exist.
	///
	/// # Errors
	///
	/// If another user of this lock panicked while holding the lock, then
	/// this call will return an error instead.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, Poisonable, ThreadKey};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let mut mutex = Poisonable::new(Mutex::new(0));
	/// *mutex.get_mut().unwrap() = 10;
	/// assert_eq!(*mutex.lock(key).unwrap(), 10);
	/// ```
	pub fn get_mut(&mut self) -> PoisonResult<&mut L::Inner> {
		if self.is_poisoned() {
			Err(PoisonError::new(self.inner.as_mut()))
		} else {
			Ok(self.inner.as_mut())
		}
	}
}

impl<L: Lockable + RawLock> RefUnwindSafe for Poisonable<L> {}
impl<L: Lockable + RawLock> UnwindSafe for Poisonable<L> {}
