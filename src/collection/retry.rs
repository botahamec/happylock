use std::cell::RefCell;
use std::collections::HashSet;
use std::marker::PhantomData;

use crate::collection::utils;
use crate::handle_unwind::handle_unwind;
use crate::lockable::{
	Lockable, LockableGetMut, LockableIntoInner, OwnedLockable, RawLock, Sharable,
};
use crate::Keyable;

use super::{LockGuard, RetryingLockCollection};

/// Get all raw locks in the collection
fn get_locks<L: Lockable>(data: &L) -> Vec<&dyn RawLock> {
	let mut locks = Vec::new();
	data.get_ptrs(&mut locks);
	locks
}

/// Checks that a collection contains no duplicate references to a lock.
fn contains_duplicates<L: Lockable>(data: L) -> bool {
	let mut locks = Vec::new();
	data.get_ptrs(&mut locks);
	// cast to *const () so that the v-table pointers are not used for hashing
	let locks = locks.into_iter().map(|l| (&raw const *l).cast::<()>());

	let mut locks_set = HashSet::with_capacity(locks.len());
	for lock in locks {
		if !locks_set.insert(lock) {
			return true;
		}
	}

	false
}

unsafe impl<L: Lockable> RawLock for RetryingLockCollection<L> {
	#[mutants::skip] // this should never run
	fn poison(&self) {
		let locks = get_locks(&self.data);
		for lock in locks {
			lock.poison();
		}
	}

	unsafe fn raw_lock(&self) {
		let mut first_index = 0;
		let locks = get_locks(&self.data);

		if locks.is_empty() {
			// this probably prevents a panic later
			return;
		}

		// these will be unlocked in case of a panic
		let locked = RefCell::new(Vec::with_capacity(locks.len()));
		handle_unwind(
			|| unsafe {
				'outer: loop {
					// This prevents us from entering a spin loop waiting for
					// the same lock to be unlocked
					// safety: we have the thread key
					locks[first_index].raw_lock();
					for (i, lock) in locks.iter().enumerate() {
						if i == first_index {
							// we've already locked this one
							continue;
						}

						// If the lock has been killed, then this returns false
						// instead of panicking. This sounds like a problem, but if
						// it does return false, then the lock function is called
						// immediately after, causing a panic
						// safety: we have the thread key
						if lock.raw_try_lock() {
							locked.borrow_mut().push(*lock)
						} else {
							for lock in locked.borrow().iter() {
								// safety: we already locked all of these
								lock.raw_unlock();
							}
							// these are no longer locked
							locked.borrow_mut().clear();

							if first_index >= i {
								// safety: this is already locked and can't be unlocked
								//         by the previous loop
								locks[first_index].raw_unlock();
							}

							// call lock on this to prevent a spin loop
							first_index = i;
							continue 'outer;
						}
					}

					// safety: we locked all the data
					break;
				}
			},
			|| utils::attempt_to_recover_locks_from_panic(&locked),
		)
	}

	unsafe fn raw_try_lock(&self) -> bool {
		let locks = get_locks(&self.data);

		if locks.is_empty() {
			// this is an interesting case, but it doesn't give us access to
			// any data, and can't possibly cause a deadlock
			return true;
		}

		// these will be unlocked in case of a panic
		let locked = RefCell::new(Vec::with_capacity(locks.len()));
		handle_unwind(
			|| unsafe {
				for (i, lock) in locks.iter().enumerate() {
					// safety: we have the thread key
					if lock.raw_try_lock() {
						locked.borrow_mut().push(*lock);
					} else {
						for lock in locks.iter().take(i) {
							// safety: we already locked all of these
							lock.raw_unlock();
						}
						return false;
					}
				}

				true
			},
			|| utils::attempt_to_recover_locks_from_panic(&locked),
		)
	}

	unsafe fn raw_unlock(&self) {
		let locks = get_locks(&self.data);

		for lock in locks {
			lock.raw_unlock();
		}
	}

	unsafe fn raw_read(&self) {
		let mut first_index = 0;
		let locks = get_locks(&self.data);

		if locks.is_empty() {
			// this probably prevents a panic later
			return;
		}

		let locked = RefCell::new(Vec::with_capacity(locks.len()));
		handle_unwind(
			|| 'outer: loop {
				// safety: we have the thread key
				locks[first_index].raw_read();
				for (i, lock) in locks.iter().enumerate() {
					if i == first_index {
						continue;
					}

					// safety: we have the thread key
					if lock.raw_try_read() {
						locked.borrow_mut().push(*lock);
					} else {
						for lock in locked.borrow().iter() {
							// safety: we already locked all of these
							lock.raw_unlock_read();
						}
						// these are no longer locked
						locked.borrow_mut().clear();

						if first_index >= i {
							// safety: this is already locked and can't be unlocked
							//         by the previous loop
							locks[first_index].raw_unlock_read();
						}

						// don't go into a spin loop, wait for this one to lock
						first_index = i;
						continue 'outer;
					}
				}

				// safety: we locked all the data
				break;
			},
			|| utils::attempt_to_recover_reads_from_panic(&locked),
		)
	}

	unsafe fn raw_try_read(&self) -> bool {
		let locks = get_locks(&self.data);

		if locks.is_empty() {
			// this is an interesting case, but it doesn't give us access to
			// any data, and can't possibly cause a deadlock
			return true;
		}

		let locked = RefCell::new(Vec::with_capacity(locks.len()));
		handle_unwind(
			|| unsafe {
				for (i, lock) in locks.iter().enumerate() {
					// safety: we have the thread key
					if lock.raw_try_read() {
						locked.borrow_mut().push(*lock);
					} else {
						for lock in locks.iter().take(i) {
							// safety: we already locked all of these
							lock.raw_unlock_read();
						}
						return false;
					}
				}

				true
			},
			|| utils::attempt_to_recover_reads_from_panic(&locked),
		)
	}

	unsafe fn raw_unlock_read(&self) {
		let locks = get_locks(&self.data);

		for lock in locks {
			lock.raw_unlock_read();
		}
	}
}

unsafe impl<L: Lockable> Lockable for RetryingLockCollection<L> {
	type Guard<'g>
		= L::Guard<'g>
	where
		Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		self.data.get_ptrs(ptrs)
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		self.data.guard()
	}
}

unsafe impl<L: Sharable> Sharable for RetryingLockCollection<L> {
	type ReadGuard<'g>
		= L::ReadGuard<'g>
	where
		Self: 'g;

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		self.data.read_guard()
	}
}

unsafe impl<L: OwnedLockable> OwnedLockable for RetryingLockCollection<L> {}

impl<L: LockableGetMut> LockableGetMut for RetryingLockCollection<L> {
	type Inner<'a>
		= L::Inner<'a>
	where
		Self: 'a;

	fn get_mut(&mut self) -> Self::Inner<'_> {
		self.data.get_mut()
	}
}

impl<L: LockableIntoInner> LockableIntoInner for RetryingLockCollection<L> {
	type Inner = L::Inner;

	fn into_inner(self) -> Self::Inner {
		self.data.into_inner()
	}
}

impl<L> IntoIterator for RetryingLockCollection<L>
where
	L: IntoIterator,
{
	type Item = <L as IntoIterator>::Item;
	type IntoIter = <L as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

impl<'a, L> IntoIterator for &'a RetryingLockCollection<L>
where
	&'a L: IntoIterator,
{
	type Item = <&'a L as IntoIterator>::Item;
	type IntoIter = <&'a L as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

impl<'a, L> IntoIterator for &'a mut RetryingLockCollection<L>
where
	&'a mut L: IntoIterator,
{
	type Item = <&'a mut L as IntoIterator>::Item;
	type IntoIter = <&'a mut L as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

impl<L: OwnedLockable, I: FromIterator<L> + OwnedLockable> FromIterator<L>
	for RetryingLockCollection<I>
{
	fn from_iter<T: IntoIterator<Item = L>>(iter: T) -> Self {
		let iter: I = iter.into_iter().collect();
		Self::new(iter)
	}
}

impl<E: OwnedLockable + Extend<L>, L: OwnedLockable> Extend<L> for RetryingLockCollection<E> {
	fn extend<T: IntoIterator<Item = L>>(&mut self, iter: T) {
		self.data.extend(iter)
	}
}

impl<T, L: AsRef<T>> AsRef<T> for RetryingLockCollection<L> {
	fn as_ref(&self) -> &T {
		self.data.as_ref()
	}
}

impl<T, L: AsMut<T>> AsMut<T> for RetryingLockCollection<L> {
	fn as_mut(&mut self) -> &mut T {
		self.data.as_mut()
	}
}

impl<L: OwnedLockable + Default> Default for RetryingLockCollection<L> {
	fn default() -> Self {
		Self::new(L::default())
	}
}

impl<L: OwnedLockable> From<L> for RetryingLockCollection<L> {
	fn from(value: L) -> Self {
		Self::new(value)
	}
}

impl<L: OwnedLockable> RetryingLockCollection<L> {
	/// Creates a new collection of owned locks.
	///
	/// Because the locks are owned, there's no need to do any checks for
	/// duplicate values. The locks also don't need to be sorted by memory
	/// address because they aren't used anywhere else.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::Mutex;
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = RetryingLockCollection::new(data);
	/// ```
	#[must_use]
	pub const fn new(data: L) -> Self {
		Self { data }
	}
}

impl<'a, L: OwnedLockable> RetryingLockCollection<&'a L> {
	/// Creates a new collection of owned locks.
	///
	/// Because the locks are owned, there's no need to do any checks for
	/// duplicate values.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::Mutex;
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = RetryingLockCollection::new_ref(&data);
	/// ```
	#[must_use]
	pub const fn new_ref(data: &'a L) -> Self {
		Self { data }
	}
}

impl<L> RetryingLockCollection<L> {
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
	/// use happylock::Mutex;
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let data1 = Mutex::new(0);
	/// let data2 = Mutex::new("");
	///
	/// // safety: data1 and data2 refer to distinct mutexes
	/// let data = (&data1, &data2);
	/// let lock = unsafe { RetryingLockCollection::new_unchecked(&data) };
	/// ```
	#[must_use]
	pub const unsafe fn new_unchecked(data: L) -> Self {
		Self { data }
	}

	/// Gets an immutable reference to the underlying collection.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey};
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let data = (Mutex::new(42), Mutex::new(""));
	/// let lock = RetryingLockCollection::new(data);
	///
	/// let key = ThreadKey::get().unwrap();
	/// let inner = lock.child();
	/// let guard = inner.0.lock(key);
	/// assert_eq!(*guard, 42);
	/// ```
	#[must_use]
	pub const fn child(&self) -> &L {
		&self.data
	}

	/// Gets a mutable reference to the underlying collection.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey};
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let data = (Mutex::new(42), Mutex::new(""));
	/// let mut lock = RetryingLockCollection::new(data);
	///
	/// let key = ThreadKey::get().unwrap();
	/// let mut inner = lock.child_mut();
	/// let guard = inner.0.get_mut();
	/// assert_eq!(*guard, 42);
	/// ```
	#[must_use]
	pub fn child_mut(&mut self) -> &mut L {
		&mut self.data
	}

	/// Gets the underlying collection, consuming this collection.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey};
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let data = (Mutex::new(42), Mutex::new(""));
	/// let lock = RetryingLockCollection::new(data);
	///
	/// let key = ThreadKey::get().unwrap();
	/// let inner = lock.into_child();
	/// let guard = inner.0.lock(key);
	/// assert_eq!(*guard, 42);
	/// ```
	#[must_use]
	pub fn into_child(self) -> L {
		self.data
	}
}

impl<L: Lockable> RetryingLockCollection<L> {
	/// Creates a new collection of locks.
	///
	/// This returns `None` if any locks are found twice in the given
	/// collection.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::Mutex;
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let data1 = Mutex::new(0);
	/// let data2 = Mutex::new("");
	///
	/// // data1 and data2 refer to distinct mutexes, so this won't panic
	/// let data = (&data1, &data2);
	/// let lock = RetryingLockCollection::try_new(&data).unwrap();
	/// ```
	#[must_use]
	pub fn try_new(data: L) -> Option<Self> {
		(!contains_duplicates(&data)).then_some(Self { data })
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
	/// use happylock::{Mutex, ThreadKey};
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = RetryingLockCollection::new(data);
	///
	/// let mut guard = lock.lock(key);
	/// *guard.0 += 1;
	/// *guard.1 = "1";
	/// ```
	pub fn lock<'g, 'key: 'g, Key: Keyable + 'key>(
		&'g self,
		key: Key,
	) -> LockGuard<'key, L::Guard<'g>, Key> {
		unsafe {
			// safety: we're taking the thread key
			self.raw_lock();

			LockGuard {
				// safety: we just locked the collection
				guard: self.guard(),
				key,
				_phantom: PhantomData,
			}
		}
	}

	/// Attempts to lock the without blocking.
	///
	/// If the access could not be granted at this time, then `Err` is
	/// returned. Otherwise, an RAII guard is returned which will release the
	/// locks when it is dropped.
	///
	/// # Errors
	///
	/// If any of the locks in the collection are already locked, then an error
	/// is returned containing the given key.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey};
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = RetryingLockCollection::new(data);
	///
	/// match lock.try_lock(key) {
	///     Ok(mut guard) => {
	///         *guard.0 += 1;
	///         *guard.1 = "1";
	///     },
	///     Err(_) => unreachable!(),
	/// };
	///
	/// ```
	pub fn try_lock<'g, 'key: 'g, Key: Keyable + 'key>(
		&'g self,
		key: Key,
	) -> Result<LockGuard<'key, L::Guard<'g>, Key>, Key> {
		unsafe {
			// safety: we're taking the thread key
			if self.raw_try_lock() {
				Ok(LockGuard {
					// safety: we just succeeded in locking everything
					guard: self.guard(),
					key,
					_phantom: PhantomData,
				})
			} else {
				Err(key)
			}
		}
	}

	/// Unlocks the underlying lockable data type, returning the key that's
	/// associated with it.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey};
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = RetryingLockCollection::new(data);
	///
	/// let mut guard = lock.lock(key);
	/// *guard.0 += 1;
	/// *guard.1 = "1";
	/// let key = RetryingLockCollection::<(Mutex<i32>, Mutex<&str>)>::unlock(guard);
	/// ```
	pub fn unlock<'key, Key: Keyable + 'key>(guard: LockGuard<'key, L::Guard<'_>, Key>) -> Key {
		drop(guard.guard);
		guard.key
	}
}

impl<L: Sharable> RetryingLockCollection<L> {
	/// Locks the collection, so that other threads can still read from it
	///
	/// This function returns a guard that can be used to access the underlying
	/// data immutably. When the guard is dropped, the locks in the collection
	/// are also dropped.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey};
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (RwLock::new(0), RwLock::new(""));
	/// let lock = RetryingLockCollection::new(data);
	///
	/// let mut guard = lock.read(key);
	/// assert_eq!(*guard.0, 0);
	/// assert_eq!(*guard.1, "");
	/// ```
	pub fn read<'g, 'key: 'g, Key: Keyable + 'key>(
		&'g self,
		key: Key,
	) -> LockGuard<'key, L::ReadGuard<'g>, Key> {
		unsafe {
			// safety: we're taking the thread key
			self.raw_read();

			LockGuard {
				// safety: we just locked the collection
				guard: self.read_guard(),
				key,
				_phantom: PhantomData,
			}
		}
	}

	/// Attempts to lock the without blocking, in such a way that other threads
	/// can still read from the collection.
	///
	/// If the access could not be granted at this time, then `Err` is
	/// returned. Otherwise, an RAII guard is returned which will release the
	/// shared access when it is dropped.
	///
	/// # Errors
	///
	/// If shared access cannot be acquired at this time, then an error is
	/// returned containing the given key.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey};
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (RwLock::new(5), RwLock::new("6"));
	/// let lock = RetryingLockCollection::new(data);
	///
	/// match lock.try_read(key) {
	///     Some(mut guard) => {
	///         assert_eq!(*guard.0, 5);
	///         assert_eq!(*guard.1, "6");
	///     },
	///     None => unreachable!(),
	/// };
	///
	/// ```
	pub fn try_read<'g, 'key: 'g, Key: Keyable + 'key>(
		&'g self,
		key: Key,
	) -> Option<LockGuard<'key, L::ReadGuard<'g>, Key>> {
		unsafe {
			// safety: we're taking the thread key
			self.raw_try_lock().then(|| LockGuard {
				// safety: we just succeeded in locking everything
				guard: self.read_guard(),
				key,
				_phantom: PhantomData,
			})
		}
	}

	/// Unlocks the underlying lockable data type, returning the key that's
	/// associated with it.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey};
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (RwLock::new(0), RwLock::new(""));
	/// let lock = RetryingLockCollection::new(data);
	///
	/// let mut guard = lock.read(key);
	/// let key = RetryingLockCollection::<(RwLock<i32>, RwLock<&str>)>::unlock_read(guard);
	/// ```
	pub fn unlock_read<'key, Key: Keyable + 'key>(
		guard: LockGuard<'key, L::ReadGuard<'_>, Key>,
	) -> Key {
		drop(guard.guard);
		guard.key
	}
}

impl<L: LockableGetMut> RetryingLockCollection<L> {
	/// Gets a mutable reference to the data behind this
	/// `RetryingLockCollection`.
	///
	/// Since this call borrows the `RetryingLockCollection` mutably, no actual
	/// locking needs to take place - the mutable borrow statically guarantees
	/// no locks exist.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, LockCollection};
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let mut mutex = RetryingLockCollection::new([Mutex::new(0), Mutex::new(0)]);
	/// assert_eq!(mutex.get_mut(), [&mut 0, &mut 0]);
	/// ```
	pub fn get_mut(&mut self) -> L::Inner<'_> {
		LockableGetMut::get_mut(self)
	}
}

impl<L: LockableIntoInner> RetryingLockCollection<L> {
	/// Consumes this `RetryingLockCollection`, returning the underlying data.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, LockCollection};
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let mutex = RetryingLockCollection::new([Mutex::new(0), Mutex::new(0)]);
	/// assert_eq!(mutex.into_inner(), [0, 0]);
	/// ```
	pub fn into_inner(self) -> L::Inner {
		LockableIntoInner::into_inner(self)
	}
}

impl<'a, L: 'a> RetryingLockCollection<L>
where
	&'a L: IntoIterator,
{
	/// Returns an iterator over references to each value in the collection.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey};
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = [Mutex::new(26), Mutex::new(1)];
	/// let lock = RetryingLockCollection::new(data);
	///
	/// let mut iter = lock.iter();
	/// let mutex = iter.next().unwrap();
	/// let guard = mutex.lock(key);
	///
	/// assert_eq!(*guard, 26);
	/// ```
	#[must_use]
	pub fn iter(&'a self) -> <&'a L as IntoIterator>::IntoIter {
		self.into_iter()
	}
}

impl<'a, L: 'a> RetryingLockCollection<L>
where
	&'a mut L: IntoIterator,
{
	/// Returns an iterator over mutable references to each value in the
	/// collection.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey};
	/// use happylock::collection::RetryingLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = [Mutex::new(26), Mutex::new(1)];
	/// let mut lock = RetryingLockCollection::new(data);
	///
	/// let mut iter = lock.iter_mut();
	/// let mutex = iter.next().unwrap();
	///
	/// assert_eq!(*mutex.as_mut(), 26);
	/// ```
	#[must_use]
	pub fn iter_mut(&'a mut self) -> <&'a mut L as IntoIterator>::IntoIter {
		self.into_iter()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::collection::BoxedLockCollection;
	use crate::{Mutex, RwLock, ThreadKey};

	#[test]
	fn nonduplicate_lock_references_are_allowed() {
		let mutex1 = Mutex::new(0);
		let mutex2 = Mutex::new(0);
		assert!(RetryingLockCollection::try_new([&mutex1, &mutex2]).is_some());
	}

	#[test]
	fn duplicate_lock_references_are_disallowed() {
		let mutex = Mutex::new(0);
		assert!(RetryingLockCollection::try_new([&mutex, &mutex]).is_none());
	}

	#[test]
	fn locks_all_inner_mutexes() {
		let key = ThreadKey::get().unwrap();
		let mutex1 = Mutex::new(0);
		let mutex2 = Mutex::new(0);
		let collection = RetryingLockCollection::try_new([&mutex1, &mutex2]).unwrap();

		let guard = collection.lock(key);

		assert!(mutex1.is_locked());
		assert!(mutex2.is_locked());

		drop(guard);
	}

	#[test]
	fn locks_all_inner_rwlocks() {
		let key = ThreadKey::get().unwrap();
		let rwlock1 = RwLock::new(0);
		let rwlock2 = RwLock::new(0);
		let collection = RetryingLockCollection::try_new([&rwlock1, &rwlock2]).unwrap();
		// TODO Poisonable::read

		let guard = collection.read(key);

		assert!(rwlock1.is_locked());
		assert!(rwlock2.is_locked());

		drop(guard);
	}

	#[test]
	fn works_with_other_collections() {
		let key = ThreadKey::get().unwrap();
		let mutex1 = Mutex::new(0);
		let mutex2 = Mutex::new(0);
		let collection = BoxedLockCollection::try_new(
			RetryingLockCollection::try_new([&mutex1, &mutex2]).unwrap(),
		)
		.unwrap();

		let guard = collection.lock(key);

		assert!(mutex1.is_locked());
		assert!(mutex2.is_locked());
		drop(guard);
	}

	#[test]
	fn extend_collection() {
		let mutex1 = Mutex::new(0);
		let mutex2 = Mutex::new(0);
		let mut collection = RetryingLockCollection::new(vec![mutex1]);

		collection.extend([mutex2]);

		assert_eq!(collection.into_inner().len(), 2);
	}
}
