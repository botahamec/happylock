use std::marker::PhantomData;
use std::panic::{RefUnwindSafe, UnwindSafe};

use crate::lockable::{
	Lockable, LockableAsMut, LockableIntoInner, OwnedLockable, RawLock, Sharable,
};
use crate::Keyable;

use super::{
	PoisonError, PoisonFlag, PoisonGuard, PoisonRef, PoisonResult, Poisonable,
	TryLockPoisonableError, TryLockPoisonableResult,
};

unsafe impl<L: Lockable + RawLock> RawLock for Poisonable<L> {
	unsafe fn lock(&self) {
		self.inner.lock()
	}

	unsafe fn try_lock(&self) -> bool {
		self.inner.try_lock()
	}

	unsafe fn unlock(&self) {
		self.inner.unlock()
	}

	unsafe fn read(&self) {
		self.inner.read()
	}

	unsafe fn try_read(&self) -> bool {
		self.inner.try_read()
	}

	unsafe fn unlock_read(&self) {
		self.inner.unlock_read()
	}
}

unsafe impl<L: Lockable> Lockable for Poisonable<L> {
	type Guard<'g>
		= PoisonResult<PoisonRef<'g, L::Guard<'g>>>
	where
		Self: 'g;

	type ReadGuard<'g>
		= PoisonResult<PoisonRef<'g, L::ReadGuard<'g>>>
	where
		Self: 'g;

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

unsafe impl<L: Sharable> Sharable for Poisonable<L> {}
unsafe impl<L: OwnedLockable> OwnedLockable for Poisonable<L> {}

impl<L> From<L> for Poisonable<L> {
	fn from(value: L) -> Self {
		Self::new(value)
	}
}

impl<L> Poisonable<L> {
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
	/// This consumes the `Poisonable` and returns ownership of the lock, which
	/// means that the `Poisonable` can still be `RefUnwindSafe`.
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
	/// assert_eq!(mutex.into_child().unwrap().into_inner(), 0);
	/// ```
	pub fn into_child(self) -> PoisonResult<L> {
		if self.is_poisoned() {
			Err(PoisonError::new(self.inner))
		} else {
			Ok(self.inner)
		}
	}

	/// Returns a mutable reference to the underlying lock.
	///
	/// This can be implemented while still being `RefUnwindSafe` because
	/// it requires a mutable reference.
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
	/// *mutex.child_mut().unwrap().as_mut() = 10;
	/// assert_eq!(*mutex.lock(key).unwrap(), 10);
	/// ```
	pub fn child_mut(&mut self) -> PoisonResult<&mut L> {
		if self.is_poisoned() {
			Err(PoisonError::new(&mut self.inner))
		} else {
			Ok(&mut self.inner)
		}
	}

	// NOTE: `child_ref` isn't implemented because it would make this not `RefUnwindSafe`
	//
}

impl<L: Lockable> Poisonable<L> {
	/// Creates a guard for the poisonable, without locking it
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
}

impl<L: Lockable + RawLock> Poisonable<L> {
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
	/// this call will return an error once the mutex is acquired.
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
}

impl<L: Sharable + RawLock> Poisonable<L> {
	unsafe fn read_guard<'flag, 'key, Key: Keyable + 'key>(
		&'flag self,
		key: Key,
	) -> PoisonResult<PoisonGuard<'flag, 'key, L::ReadGuard<'flag>, Key>> {
		let guard = PoisonGuard {
			guard: PoisonRef::new(&self.poisoned, self.inner.read_guard()),
			key,
			_phantom: PhantomData,
		};

		if self.is_poisoned() {
			return Err(PoisonError::new(guard));
		}

		Ok(guard)
	}

	/// Locks with shared read access, blocking the current thread until it can
	/// be acquired.
	///
	/// This function will block the current thread until there are no writers
	/// which hold the lock. This method does not provide any guarantee with
	/// respect to the ordering of contentious readers or writers will acquire
	/// the lock.
	///
	/// # Errors
	///
	/// If another use of this lock panicked while holding the lock, then
	/// this call will return an error once the lock is acquired.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use std::thread;
	///
	/// use happylock::{RwLock, Poisonable, ThreadKey};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = Arc::new(Poisonable::new(RwLock::new(0)));
	/// let c_lock = Arc::clone(&lock);
	///
	/// let n = lock.read(key).unwrap();
	/// assert_eq!(*n, 0);
	///
	/// thread::spawn(move || {
	///     let key = ThreadKey::get().unwrap();
	///     assert!(c_lock.read(key).is_ok());
	/// }).join().expect("thread::spawn failed");
	/// ```
	pub fn read<'flag, 'key, Key: Keyable + 'key>(
		&'flag self,
		key: Key,
	) -> PoisonResult<PoisonGuard<'flag, 'key, L::ReadGuard<'flag>, Key>> {
		unsafe {
			self.inner.read();
			self.read_guard(key)
		}
	}

	/// Attempts to acquire the lock with shared read access.
	///
	/// If the access could not be granted at this time, then `Err` is returned.
	/// Otherwise, an RAII guard is returned which will release the shared access
	/// when it is dropped.
	///
	/// This function does not block.
	///
	/// This function does not provide any guarantees with respect to the ordering
	/// of whether contentious readers or writers will acquire the lock first.
	///
	/// # Errors
	///
	/// This function will return the [`Poisoned`] error if the lock is
	/// poisoned. A [`Poisonable`] is poisoned whenever a writer panics while
	/// holding an exclusive lock. `Poisoned` will only be returned if the lock
	/// would have otherwise been acquired.
	///
	/// This function will return the [`WouldBlock`] error if the lock could
	/// not be acquired because it was already locked exclusively.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Poisonable, RwLock, ThreadKey};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = Poisonable::new(RwLock::new(1));
	///
	/// match lock.try_read(key) {
	///     Ok(n) => assert_eq!(*n, 1),
	///     Err(_) => unreachable!(),
	/// };
	/// ```
	///
	/// [`Poisoned`]: `TryLockPoisonableError::Poisoned`
	/// [`WouldBlock`]: `TryLockPoisonableError::WouldBlock`
	pub fn try_read<'flag, 'key, Key: Keyable + 'key>(
		&'flag self,
		key: Key,
	) -> TryLockPoisonableResult<'flag, 'key, L::ReadGuard<'flag>, Key> {
		unsafe {
			if self.inner.try_read() {
				Ok(self.read_guard(key)?)
			} else {
				Err(TryLockPoisonableError::WouldBlock(key))
			}
		}
	}

	/// Consumes the [`PoisonGuard`], and consequently unlocks its underlying lock.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{ThreadKey, RwLock, Poisonable};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = Poisonable::new(RwLock::new(0));
	///
	/// let mut guard = lock.read(key).unwrap();
	/// let key = Poisonable::<RwLock<_>>::unlock_read(guard);
	/// ```
	pub fn unlock_read<'flag, 'key, Key: Keyable + 'key>(
		guard: PoisonGuard<'flag, 'key, L::ReadGuard<'flag>, Key>,
	) -> Key {
		drop(guard.guard);
		guard.key
	}
}

impl<L: LockableIntoInner> Poisonable<L> {
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
	pub fn get_mut(&mut self) -> PoisonResult<L::Inner<'_>> {
		if self.is_poisoned() {
			Err(PoisonError::new(self.inner.as_mut()))
		} else {
			Ok(self.inner.as_mut())
		}
	}
}

impl<L: UnwindSafe> RefUnwindSafe for Poisonable<L> {}
impl<L: UnwindSafe> UnwindSafe for Poisonable<L> {}
