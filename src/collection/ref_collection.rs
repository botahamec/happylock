use std::marker::PhantomData;

use crate::{key::Keyable, lockable::Lock, Lockable, OwnedLockable};

use super::{LockGuard, RefLockCollection};

#[must_use]
fn get_locks<L: Lockable>(data: &L) -> Vec<&dyn Lock> {
	let mut locks = Vec::new();
	data.get_ptrs(&mut locks);
	locks.sort_by_key(|lock| std::ptr::from_ref(*lock));
	locks
}

/// returns `true` if the sorted list contains a duplicate
#[must_use]
fn contains_duplicates(l: &[&dyn Lock]) -> bool {
	l.windows(2).any(|window| {
		std::ptr::addr_eq(std::ptr::from_ref(window[0]), std::ptr::from_ref(window[1]))
	})
}

impl<'a, L: Lockable> AsRef<L> for RefLockCollection<'a, L> {
	fn as_ref(&self) -> &L {
		self.data
	}
}

impl<'a, L: Lockable> AsRef<Self> for RefLockCollection<'a, L> {
	fn as_ref(&self) -> &Self {
		self
	}
}

impl<'a, L: Lockable> AsMut<Self> for RefLockCollection<'a, L> {
	fn as_mut(&mut self) -> &mut Self {
		self
	}
}

impl<'a, L> IntoIterator for &'a RefLockCollection<'a, L>
where
	&'a L: IntoIterator,
{
	type Item = <&'a L as IntoIterator>::Item;
	type IntoIter = <&'a L as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

impl<'a, L: OwnedLockable> RefLockCollection<'a, L> {
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
	/// let lock = LockCollection::new(&data);
	/// ```
	#[must_use]
	pub fn new(data: &'a L) -> RefLockCollection<L> {
		RefLockCollection {
			locks: get_locks(data),
			data,
		}
	}
}

impl<'a, L: Lockable> RefLockCollection<'a, L> {
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
	pub unsafe fn new_unchecked(data: &'a L) -> Self {
		Self {
			data,
			locks: get_locks(data),
		}
	}

	/// Creates a new collection of locks.
	///
	/// This returns `None` if any locks are found twice in the given
	/// collection.
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
	pub fn try_new(data: &'a L) -> Option<Self> {
		let locks = get_locks(data);
		if contains_duplicates(&locks) {
			return None;
		}

		Some(Self { locks, data })
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
		for lock in &self.locks {
			// safety: we have the thread key
			unsafe { lock.lock() };
		}

		LockGuard {
			// safety: we've already acquired the lock
			guard: unsafe { self.data.guard() },
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
		let guard = unsafe {
			for (i, lock) in self.locks.iter().enumerate() {
				// safety: we have the thread key
				let success = lock.try_lock();

				if !success {
					for lock in &self.locks[0..i] {
						// safety: this lock was already acquired
						lock.unlock();
					}
					return None;
				}
			}

			// safety: we've acquired the locks
			self.data.guard()
		};

		Some(LockGuard {
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

impl<'a, L: 'a> RefLockCollection<'a, L>
where
	&'a L: IntoIterator,
{
	/// Returns an iterator over references to each value in the collection.
	#[must_use]
	pub fn iter(&'a self) -> <&'a L as IntoIterator>::IntoIter {
		self.into_iter()
	}
}
