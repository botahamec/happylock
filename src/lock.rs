use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};

use once_cell::sync::Lazy;
use thread_local::ThreadLocal;

static KEY: Lazy<ThreadLocal<Lock>> = Lazy::new(ThreadLocal::new);

/// The key for the current thread.
///
/// Only one of these exist per thread. To get the current thread's key, call
/// [`ThreadKey::lock`]. If the `ThreadKey` is dropped, it can be reobtained.
pub type ThreadKey = Key<'static>;

/// A dumb lock that's just a wrapper for an [`AtomicBool`].
#[derive(Debug, Default)]
pub struct Lock {
	is_locked: AtomicBool,
}

pub struct Key<'a> {
	phantom: PhantomData<*const ()>, // implement !Send and !Sync
	lock: &'a Lock,
}

impl Debug for ThreadKey {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "ThreadKey")
	}
}

impl<'a> Drop for Key<'a> {
	fn drop(&mut self) {
		unsafe { self.lock.force_unlock() }
	}
}

impl ThreadKey {
	/// Get the current thread's `ThreadKey`, if it's not already taken.
	///
	/// The first time this is called, it will successfully return a
	/// `ThreadKey`. However, future calls to this function will return
	/// [`None`], unless the key is dropped or unlocked first.
	#[must_use]
	pub fn lock() -> Option<Self> {
		KEY.get_or_default().try_lock()
	}

	/// Unlocks the `ThreadKey`.
	///
	/// After this method is called, a call to [`ThreadKey::lock`] will return
	/// this `ThreadKey`.
	pub fn unlock(key: Self) {
		drop(key);
	}
}

impl Lock {
	/// Create a new unlocked `Lock`.
	#[must_use]
	pub const fn new() -> Self {
		Self {
			is_locked: AtomicBool::new(false),
		}
	}

	/// Checks whether this `Lock` is currently locked.
	pub fn is_locked(&self) -> bool {
		self.is_locked.load(Ordering::Relaxed)
	}

	/// Attempt to lock the `Lock`.
	///
	/// If the lock is already locked, then this'll return false. If it is
	/// unlocked, it'll return true. If the lock is currently unlocked, then it
	/// will be locked after this function is called.
	///
	/// This is not a fair lock. It is not recommended to call this function
	/// repeatedly in a loop.
	pub fn try_lock(&self) -> Option<Key> {
		// safety: we just acquired the lock
		(!self.is_locked.swap(true, Ordering::Acquire)).then_some(Key {
			phantom: PhantomData,
			lock: self,
		})
	}

	/// Forcibly unlocks the `Lock`.
	///
	/// # Safety
	///
	/// This should only be called if the key to the lock has been "lost". That
	/// means the program no longer has a reference to the key.
	pub unsafe fn force_unlock(&self) {
		self.is_locked.store(false, Ordering::Release);
	}
}
