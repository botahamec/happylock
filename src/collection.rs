use std::{
	marker::PhantomData,
	ops::{Deref, DerefMut},
};

use crate::{
	key::Keyable,
	lockable::{Lockable, OwnedLockable},
};

/// returns `true` if the list contains a duplicate
fn contains_duplicates(l: &[usize]) -> bool {
	for i in 0..l.len() {
		for j in (i + 1)..l.len() {
			if l[i] == l[j] {
				return true;
			}
		}
	}

	false
}

/// A type which can be locked.
///
/// This could be a tuple of [`Lockable`] types, an array, or a `Vec`. But it
/// can be safely locked without causing a deadlock. To do this, it is very
/// important that no duplicate locks are included within.
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

impl<'a, L: OwnedLockable<'a>> LockCollection<L> {
	/// Creates a new collection of owned locks.
	///
	/// Because the locks are owned, there's no need to do any checks for
	/// duplicate values.
	pub const fn new(collection: L) -> Self {
		Self { collection }
	}

	/// Creates a new collection of owned locks.
	///
	/// Because the locks are owned, there's no need to do any checks for
	/// duplicate values.
	pub const fn new_ref(collection: &L) -> LockCollection<&L> {
		LockCollection { collection }
	}
}

impl<'a, L: Lockable<'a>> LockCollection<L> {
	/// Creates a new collection of locks.
	///
	/// This returns `None` if any locks are found twice in the given
	/// collection.
	///
	/// # Performance
	///
	/// This does a check at runtime to make sure that the collection contains
	/// no two copies of the same lock. This is an `O(n^2)` operation. Prefer
	/// [`LockCollection::new`] or [`LockCollection::new_ref`] instead.
	pub fn try_new(collection: L) -> Option<Self> {
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
