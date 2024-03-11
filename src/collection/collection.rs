use std::marker::PhantomData;

use crate::{key::Keyable, Lockable, OwnedLockable};

use super::{LockCollection, LockGuard};

/// returns `true` if the list contains a duplicate
#[must_use]
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

impl<'a, L: OwnedLockable<'a>> From<L> for LockCollection<L> {
	fn from(value: L) -> Self {
		Self::new(value)
	}
}

impl<'a, L: OwnedLockable<'a>> AsRef<L> for LockCollection<L> {
	fn as_ref(&self) -> &L {
		&self.collection
	}
}

impl<'a, L: OwnedLockable<'a>> AsMut<L> for LockCollection<L> {
	fn as_mut(&mut self) -> &mut L {
		&mut self.collection
	}
}

impl<'a, L: OwnedLockable<'a>> AsRef<Self> for LockCollection<L> {
	fn as_ref(&self) -> &Self {
		self
	}
}

impl<'a, L: OwnedLockable<'a>> AsMut<Self> for LockCollection<L> {
	fn as_mut(&mut self) -> &mut Self {
		self
	}
}

impl<L: IntoIterator> IntoIterator for LockCollection<L> {
	type Item = L::Item;
	type IntoIter = L::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.collection.into_iter()
	}
}

impl<'a, L> IntoIterator for &'a LockCollection<L>
where
	&'a L: IntoIterator,
{
	type Item = <&'a L as IntoIterator>::Item;
	type IntoIter = <&'a L as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.collection.into_iter()
	}
}

impl<'a, L> IntoIterator for &'a mut LockCollection<L>
where
	&'a mut L: IntoIterator,
{
	type Item = <&'a mut L as IntoIterator>::Item;
	type IntoIter = <&'a mut L as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.collection.into_iter()
	}
}

impl<'a, L: OwnedLockable<'a>, I: FromIterator<L> + OwnedLockable<'a>> FromIterator<L>
	for LockCollection<I>
{
	fn from_iter<T: IntoIterator<Item = L>>(iter: T) -> Self {
		let iter: I = iter.into_iter().collect();
		Self::new(iter)
	}
}

impl<'a, E: OwnedLockable<'a> + Extend<L>, L: OwnedLockable<'a>> Extend<L> for LockCollection<E> {
	fn extend<T: IntoIterator<Item = L>>(&mut self, iter: T) {
		self.collection.extend(iter)
	}
}

impl<'a, L: OwnedLockable<'a>> LockCollection<L> {
	/// Creates a new collection of owned locks.
	///
	/// Because the locks are owned, there's no need to do any checks for
	/// duplicate values.
	#[must_use]
	pub const fn new(collection: L) -> Self {
		Self { collection }
	}

	/// Creates a new collection of owned locks.
	///
	/// Because the locks are owned, there's no need to do any checks for
	/// duplicate values.
	#[must_use]
	pub const fn new_ref(collection: &L) -> LockCollection<&L> {
		LockCollection { collection }
	}
}

impl<L> LockCollection<L> {
	/// Creates a new collections of locks.
	///
	/// # Safety
	///
	/// This results in undefined behavior if any locks are presented twice
	/// within this collection.
	#[must_use]
	pub const unsafe fn new_unchecked(collection: L) -> Self {
		Self { collection }
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
	#[must_use]
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

impl<'a, L: 'a> LockCollection<L>
where
	&'a L: IntoIterator,
{
	/// Returns an iterator over references to each value in the collection.
	pub fn iter(&'a self) -> <&'a L as IntoIterator>::IntoIter {
		self.into_iter()
	}
}

impl<'a, L: 'a> LockCollection<L>
where
	&'a mut L: IntoIterator,
{
	/// Returns an iterator over mutable references to each value in the
	/// collection.
	pub fn iter_mut(&'a mut self) -> <&'a mut L as IntoIterator>::IntoIter {
		self.into_iter()
	}
}
