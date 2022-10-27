use std::sync::atomic::{AtomicBool, Ordering};

/// A dumb lock that's just a wrapper for an [`AtomicBool`].
#[derive(Debug, Default)]
pub struct Lock {
	is_locked: AtomicBool,
}

/// A key for a lock.
///
/// This key is needed in order to unlock a [`Lock`]. The [`Lock`] is
/// automatically unlocked if this key is dropped.
#[derive(Debug)]
pub struct Key<'a> {
	lock: &'a Lock,
}

impl<'a> Key<'a> {
	/// Create a key to a lock.
	const fn new(lock: &'a Lock) -> Self {
		Self { lock }
	}
}

impl<'a> Drop for Key<'a> {
	fn drop(&mut self) {
		// safety: this key will soon be destroyed
		unsafe { self.lock.force_unlock() }
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

	/// Attempt to lock the `Lock`.
	///
	/// If the lock is already locked, then this'll return false. If it is
	/// unlocked, it'll return true. If the lock is currently unlocked, then it
	/// will be locked after this function is called.
	///
	/// This is not a fair lock. It is not recommended to call this function
	/// repeatedly in a loop.
	pub fn try_lock(&self) -> Option<Key> {
		(!self.is_locked.fetch_or(true, Ordering::Acquire)).then_some(Key::new(self))
	}

	/// Unlock the lock, without a key.
	///
	/// # Safety
	///
	/// This should only be called if the key to the lock has been "lost". That
	/// means the program no longer has a reference to the key, but it has not
	/// been dropped.
	pub unsafe fn force_unlock(&self) {
		self.is_locked.store(false, Ordering::Release);
	}

	/// Unlock the lock, consuming its key.
	pub fn unlock(key: Key) {
		drop(key);
	}
}
