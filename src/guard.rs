use std::ops::{Deref, DerefMut};

use crate::{lockable::Lockable, ThreadKey};

/// A guard for a generic [`Lockable`] type.
pub struct LockGuard<'a, L: Lockable<'a>> {
	guard: L::Output,
	key: ThreadKey,
}

impl<'a, L: Lockable<'a>> LockGuard<'a, L> {
	/// Locks the lockable type and returns a guard that can be used to access
	/// the underlying data.
	pub fn lock(lock: &'a L, key: ThreadKey) -> Self {
		Self {
			// safety: we have the thread's key
			guard: unsafe { lock.lock() },
			key,
		}
	}

	/// Attempts to lock the guard without blocking. If successful, this method
	/// returns a guard that can be used to access the data. Otherwise, the key
	/// is given back as an error.
	pub fn try_lock(lock: &'a L, key: ThreadKey) -> Result<Self, ThreadKey> {
		// safety: we have the thread's key
		match unsafe { lock.try_lock() } {
			Some(guard) => Ok(Self { guard, key }),
			None => Err(key),
		}
	}

	/// Unlocks the underlying lockable data type, returning the key that's
	/// associated with it.
	#[allow(clippy::missing_const_for_fn)]
	pub fn unlock(self) -> ThreadKey {
		L::unlock(self.guard);
		self.key
	}
}

impl<'a, L: Lockable<'a>> Deref for LockGuard<'a, L> {
	type Target = L::Output;

	fn deref(&self) -> &Self::Target {
		&self.guard
	}
}

impl<'a, L: Lockable<'a>> DerefMut for LockGuard<'a, L> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.guard
	}
}
