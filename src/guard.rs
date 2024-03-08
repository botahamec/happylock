use std::ops::{Deref, DerefMut};

use crate::{key::Keyable, lockable::Lockable};

/// A guard for a generic [`Lockable`] type.
pub struct LockGuard<'a, L: Lockable<'a>, Key: Keyable> {
	guard: L::Output,
	_key: Key,
}

impl<'a, L: Lockable<'a>, Key: Keyable> LockGuard<'a, L, Key> {
	/// Locks the lockable type and returns a guard that can be used to access
	/// the underlying data.
	pub fn lock(lock: &'a L, key: Key) -> Self {
		Self {
			// safety: we have the thread's key
			guard: unsafe { lock.lock() },
			_key: key,
		}
	}

	/// Attempts to lock the guard without blocking. If successful, this method
	/// returns a guard that can be used to access the data. Otherwise, the key
	/// is given back as an error.
	pub fn try_lock(lock: &'a L, key: Key) -> Option<Self> {
		// safety: we have the thread's key
		unsafe { lock.try_lock() }.map(|guard| Self { guard, _key: key })
	}

	/// Unlocks the underlying lockable data type, returning the key that's
	/// associated with it.
	#[allow(clippy::missing_const_for_fn)]
	pub fn unlock(self) {
		L::unlock(self.guard);
	}
}

impl<'a, L: Lockable<'a>, Key: Keyable> Deref for LockGuard<'a, L, Key> {
	type Target = L::Output;

	fn deref(&self) -> &Self::Target {
		&self.guard
	}
}

impl<'a, L: Lockable<'a>, Key: Keyable> DerefMut for LockGuard<'a, L, Key> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.guard
	}
}
