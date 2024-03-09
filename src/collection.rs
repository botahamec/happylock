use std::{
	marker::PhantomData,
	ops::{Deref, DerefMut},
};

use crate::{key::Keyable, lockable::Lockable};

fn contains_duplicates(l: &[usize]) -> bool {
	for i in 0..l.len() {
		for j in 0..l.len() {
			if i != j && l[i] == l[j] {
				return true;
			}
		}
	}

	false
}

pub struct LockCollection<L> {
	collection: L,
}

/// A guard for a generic [`Lockable`] type.
pub struct LockGuard<'a, 'key: 'a, L: Lockable<'a>, Key: Keyable + 'key> {
	guard: L::Output,
	key: Key,
	_phantom: PhantomData<&'key ()>,
}

impl<L> LockCollection<L> {
	/// Creates a new collections of locks.
	///
	/// # Safety
	///
	/// This results in undefined behavior if any locks are presented twice
	/// within this collection.
	pub const unsafe fn new_unchecked(collection: L) -> Self {
		Self { collection }
	}
}

impl<'a, L: Lockable<'a>> LockCollection<L> {
	/// Creates a new collection of locks.
	///
	/// This returns `None` if any locks are found twice in the given collection.
	pub fn new(collection: L) -> Option<Self> {
		let ptrs = collection.get_ptrs();
		if contains_duplicates(&ptrs) {
			return None;
		}

		Some(Self { collection })
	}

	/// Locks the lockable type and returns a guard that can be used to access
	/// the underlying data.
	pub fn lock<'key: 'a, Key: Keyable + 'key>(&'a self, key: Key) -> LockGuard<'a, 'key, L, Key> {
		LockGuard {
			// safety: we have the thread's key
			guard: unsafe { self.collection.lock() },
			key,
			_phantom: PhantomData,
		}
	}

	/// Attempts to lock the guard without blocking.
	///
	/// If successful, this method returns a guard that can be used to access
	/// the data. Otherwise, `None` is returned.
	pub fn try_lock<'key: 'a, Key: Keyable + 'key>(
		&'a self,
		key: Key,
	) -> Option<LockGuard<'a, 'key, L, Key>> {
		// safety: we have the thread's key
		unsafe { self.collection.try_lock() }.map(|guard| LockGuard {
			guard,
			key,
			_phantom: PhantomData,
		})
	}

	/// Unlocks the underlying lockable data type, returning the key that's
	/// associated with it.
	#[allow(clippy::missing_const_for_fn)]
	pub fn unlock<'key: 'a, Key: Keyable + 'key>(guard: LockGuard<'a, 'key, L, Key>) -> Key {
		drop(guard.guard);
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
