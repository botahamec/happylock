use std::{
	marker::PhantomData,
	ops::{Deref, DerefMut},
};

use crate::{key::Keyable, lockable::Lockable};

/// A guard for a generic [`Lockable`] type.
pub struct LockGuard<'a, 'key: 'a, L: Lockable<'a>, Key: Keyable + 'key> {
	guard: L::Output,
	key: Key,
	_phantom: PhantomData<&'key ()>,
}

impl<'a, 'key: 'a, L: Lockable<'a>, Key: Keyable> LockGuard<'a, 'key, L, Key> {
	/// Locks the lockable type and returns a guard that can be used to access
	/// the underlying data.
	pub fn lock(lock: &'a L, key: Key) -> Self {
		Self {
			// safety: we have the thread's key
			guard: unsafe { lock.lock() },
			key,
			_phantom: PhantomData,
		}
	}

	/// Attempts to lock the guard without blocking. If successful, this method
	/// returns a guard that can be used to access the data. Otherwise, the key
	/// is given back as an error.
	pub fn try_lock(lock: &'a L, key: Key) -> Option<Self> {
		// safety: we have the thread's key
		unsafe { lock.try_lock() }.map(|guard| Self {
			guard,
			key,
			_phantom: PhantomData,
		})
	}

	/// Unlocks the underlying lockable data type, returning the key that's
	/// associated with it.
	#[allow(clippy::missing_const_for_fn)]
	pub fn unlock(guard: Self) -> Key {
		L::unlock(guard.guard);
		guard.key
	}
}

impl<'a, 'key: 'a, L: Lockable<'a>, Key: Keyable> Deref for LockGuard<'a, 'key, L, Key> {
	type Target = L::Output;

	fn deref(&self) -> &Self::Target {
		&self.guard
	}
}

impl<'a, 'key: 'a, L: Lockable<'a>, Key: Keyable> DerefMut for LockGuard<'a, 'key, L, Key> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.guard
	}
}
