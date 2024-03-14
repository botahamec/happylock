use std::marker::PhantomData;

use crate::{key::Keyable, Lockable, OwnedLockable};

use super::{LockCollection, LockGuard};

/// returns `true` if the list contains a duplicate
#[must_use]
fn contains_duplicates(l: &mut [usize]) -> bool {
	l.sort_unstable();
	l.windows(2).any(|w| w[0] == w[1])
}

impl<'a, L: OwnedLockable<'a>> From<L> for LockCollection<L> {
	fn from(value: L) -> Self {
		Self::new(value)
	}
}

impl<'a, L: Lockable<'a>> AsRef<L> for LockCollection<L> {
	fn as_ref(&self) -> &L {
		&self.data
	}
}

impl<'a, L: Lockable<'a>> AsMut<L> for LockCollection<L> {
	fn as_mut(&mut self) -> &mut L {
		&mut self.data
	}
}

impl<'a, L: Lockable<'a>> AsRef<Self> for LockCollection<L> {
	fn as_ref(&self) -> &Self {
		self
	}
}

impl<'a, L: Lockable<'a>> AsMut<Self> for LockCollection<L> {
	fn as_mut(&mut self) -> &mut Self {
		self
	}
}

impl<L: IntoIterator> IntoIterator for LockCollection<L> {
	type Item = L::Item;
	type IntoIter = L::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

impl<'a, L> IntoIterator for &'a LockCollection<L>
where
	&'a L: IntoIterator,
{
	type Item = <&'a L as IntoIterator>::Item;
	type IntoIter = <&'a L as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

impl<'a, L> IntoIterator for &'a mut LockCollection<L>
where
	&'a mut L: IntoIterator,
{
	type Item = <&'a mut L as IntoIterator>::Item;
	type IntoIter = <&'a mut L as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
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
		self.data.extend(iter)
	}
}

impl<'a, L: OwnedLockable<'a>> LockCollection<L> {
	/// Creates a new collection of owned locks.
	///
	/// Because the locks are owned, there's no need to do any checks for
	/// duplicate values.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{LockCollection, Mutex};
	///
	/// let lock = LockCollection::new((Mutex::new(0), Mutex::new("")));
	/// ```
	#[must_use]
	pub const fn new(data: L) -> Self {
		Self { data }
	}

	/// Creates a new collection of owned locks.
	///
	/// Because the locks are owned, there's no need to do any checks for
	/// duplicate values.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{LockCollection, Mutex};
	///
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = LockCollection::new_ref(&data);
	/// ```
	#[must_use]
	pub const fn new_ref(data: &L) -> LockCollection<&L> {
		LockCollection { data }
	}
}

impl<L> LockCollection<L> {
	/// Creates a new collections of locks.
	///
	/// # Safety
	///
	/// This results in undefined behavior if any locks are presented twice
	/// within this collection.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{LockCollection, Mutex};
	///
	/// let data1 = Mutex::new(0);
	/// let data2 = Mutex::new("");
	///
	/// // safety: data1 and data2 refer to distinct mutexes
	/// let lock = unsafe { LockCollection::new_unchecked((&data1, &data2)) };
	/// ```
	#[must_use]
	pub const unsafe fn new_unchecked(data: L) -> Self {
		Self { data }
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
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{LockCollection, Mutex};
	///
	/// let data1 = Mutex::new(0);
	/// let data2 = Mutex::new("");
	///
	/// // data1 and data2 refer to distinct mutexes, so this won't panic
	/// let lock = LockCollection::try_new((&data1, &data2)).unwrap();
	/// ```
	#[must_use]
	pub fn try_new(data: L) -> Option<Self> {
		let mut ptrs = data.get_ptrs();
		if contains_duplicates(&mut ptrs) {
			return None;
		}

		Some(Self { data })
	}

	/// Locks the collection
	///
	/// This function returns a guard that can be used to access the underlying
	/// data. When the guard is dropped, the locks in the collection are also
	/// dropped.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{LockCollection, Mutex, ThreadKey};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = LockCollection::new((Mutex::new(0), Mutex::new("")));
	///
	/// let mut guard = lock.lock(key);
	/// *guard.0 += 1;
	/// *guard.1 = "1";
	/// ```
	pub fn lock<'key: 'a, Key: Keyable + 'key>(&'a self, key: Key) -> LockGuard<'a, 'key, L, Key> {
		LockGuard {
			// safety: we have the thread's key
			guard: unsafe { self.data.lock() },
			key,
			_phantom: PhantomData,
		}
	}

	/// Attempts to lock the without blocking.
	///
	/// If successful, this method returns a guard that can be used to access
	/// the data, and unlocks the data when it is dropped. Otherwise, `None` is
	/// returned.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{LockCollection, Mutex, ThreadKey};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = LockCollection::new((Mutex::new(0), Mutex::new("")));
	///
	/// match lock.try_lock(key) {
	///     Some(mut guard) => {
	///         *guard.0 += 1;
	///         *guard.1 = "1";
	///     },
	///     None => unreachable!(),
	/// };
	///
	/// ```
	pub fn try_lock<'key: 'a, Key: Keyable + 'key>(
		&'a self,
		key: Key,
	) -> Option<LockGuard<'a, 'key, L, Key>> {
		// safety: we have the thread's key
		unsafe { self.data.try_lock() }.map(|guard| LockGuard {
			guard,
			key,
			_phantom: PhantomData,
		})
	}

	/// Unlocks the underlying lockable data type, returning the key that's
	/// associated with it.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{LockCollection, Mutex, ThreadKey};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let lock = LockCollection::new((Mutex::new(0), Mutex::new("")));
	///
	/// let mut guard = lock.lock(key);
	/// *guard.0 += 1;
	/// *guard.1 = "1";
	/// let key = LockCollection::unlock(guard);
	/// ```
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
