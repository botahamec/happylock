use std::cell::Cell;
use std::collections::HashSet;

use crate::collection::utils;
use crate::handle_unwind::handle_unwind;
use crate::lockable::{
	Lockable, LockableGetMut, LockableIntoInner, OwnedLockable, RawLock, Sharable,
};
use crate::{Keyable, ThreadKey};

use super::utils::{
	attempt_to_recover_reads_from_panic, attempt_to_recover_writes_from_panic, get_locks_unsorted,
	scoped_read, scoped_try_read, scoped_try_write, scoped_write,
};
use super::{LockGuard, RetryingLockCollection};

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
	#[cfg(not(tarpaulin_include))]
	fn poison(&self) {
		let locks = get_locks_unsorted(&self.data);
		for lock in locks {
			lock.poison();
		}
	}

	unsafe fn raw_write(&self) {
		let locks = get_locks_unsorted(&self.data);

		if locks.is_empty() {
			// this probably prevents a panic later
			return;
		}

		// these will be unlocked in case of a panic
		let first_index = Cell::new(0);
		let locked = Cell::new(0);
		handle_unwind(
			|| unsafe {
				'outer: loop {
					// This prevents us from entering a spin loop waiting for
					// the same lock to be unlocked
					// safety: we have the thread key
					locks[first_index.get()].raw_write();
					for (i, lock) in locks.iter().enumerate() {
						if i == first_index.get() {
							// we've already locked this one
							continue;
						}

						// If the lock has been killed, then this returns false
						// instead of panicking. This sounds like a problem, but if
						// it does return false, then the lock function is called
						// immediately after, causing a panic
						// safety: we have the thread key
						if lock.raw_try_write() {
							locked.set(locked.get() + 1);
						} else {
							// safety: we already locked all of these
							attempt_to_recover_writes_from_panic(&locks[0..i]);
							if first_index.get() >= i {
								// safety: this is already locked and can't be
								//         unlocked by the previous loop
								locks[first_index.get()].raw_unlock_write();
							}

							// nothing is locked anymore
							locked.set(0);

							// call lock on this to prevent a spin loop
							first_index.set(i);
							continue 'outer;
						}
					}

					// safety: we locked all the data
					break;
				}
			},
			|| {
				utils::attempt_to_recover_writes_from_panic(&locks[0..locked.get()]);
				if first_index.get() >= locked.get() {
					locks[first_index.get()].raw_unlock_write();
				}
			},
		)
	}

	unsafe fn raw_try_write(&self) -> bool {
		let locks = get_locks_unsorted(&self.data);

		if locks.is_empty() {
			// this is an interesting case, but it doesn't give us access to
			// any data, and can't possibly cause a deadlock
			return true;
		}

		// these will be unlocked in case of a panic
		let locked = Cell::new(0);
		handle_unwind(
			|| unsafe {
				for (i, lock) in locks.iter().enumerate() {
					// safety: we have the thread key
					if lock.raw_try_write() {
						locked.set(locked.get() + 1);
					} else {
						// safety: we already locked all of these
						attempt_to_recover_writes_from_panic(&locks[0..i]);
						return false;
					}
				}

				true
			},
			|| utils::attempt_to_recover_writes_from_panic(&locks[0..locked.get()]),
		)
	}

	unsafe fn raw_unlock_write(&self) {
		let locks = get_locks_unsorted(&self.data);

		for lock in locks {
			lock.raw_unlock_write();
		}
	}

	unsafe fn raw_read(&self) {
		let locks = get_locks_unsorted(&self.data);

		if locks.is_empty() {
			// this probably prevents a panic later
			return;
		}

		let locked = Cell::new(0);
		let first_index = Cell::new(0);
		handle_unwind(
			|| 'outer: loop {
				// safety: we have the thread key
				locks[first_index.get()].raw_read();
				for (i, lock) in locks.iter().enumerate() {
					if i == first_index.get() {
						continue;
					}

					// safety: we have the thread key
					if lock.raw_try_read() {
						locked.set(locked.get() + 1);
					} else {
						// safety: we already locked all of these
						attempt_to_recover_reads_from_panic(&locks[0..i]);

						if first_index.get() >= i {
							// safety: this is already locked and can't be unlocked
							//         by the previous loop
							locks[first_index.get()].raw_unlock_read();
						}

						// these are no longer locked
						locked.set(0);

						// don't go into a spin loop, wait for this one to lock
						first_index.set(i);
						continue 'outer;
					}
				}

				// safety: we locked all the data
				break;
			},
			|| {
				utils::attempt_to_recover_reads_from_panic(&locks[0..locked.get()]);
				if first_index.get() >= locked.get() {
					locks[first_index.get()].raw_unlock_read();
				}
			},
		)
	}

	unsafe fn raw_try_read(&self) -> bool {
		let locks = get_locks_unsorted(&self.data);

		if locks.is_empty() {
			// this is an interesting case, but it doesn't give us access to
			// any data, and can't possibly cause a deadlock
			return true;
		}

		let locked = Cell::new(0);
		handle_unwind(
			|| unsafe {
				for (i, lock) in locks.iter().enumerate() {
					// safety: we have the thread key
					if lock.raw_try_read() {
						locked.set(locked.get() + 1);
					} else {
						// safety: we already locked all of these
						attempt_to_recover_reads_from_panic(&locks[0..i]);
						return false;
					}
				}

				true
			},
			|| utils::attempt_to_recover_reads_from_panic(&locks[0..locked.get()]),
		)
	}

	unsafe fn raw_unlock_read(&self) {
		let locks = get_locks_unsorted(&self.data);

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

	type DataMut<'a>
		= L::DataMut<'a>
	where
		Self: 'a;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		ptrs.push(self)
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		self.data.guard()
	}

	unsafe fn data_mut(&self) -> Self::DataMut<'_> {
		self.data.data_mut()
	}
}

unsafe impl<L: Sharable> Sharable for RetryingLockCollection<L> {
	type ReadGuard<'g>
		= L::ReadGuard<'g>
	where
		Self: 'g;

	type DataRef<'a>
		= L::DataRef<'a>
	where
		Self: 'a;

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		self.data.read_guard()
	}

	unsafe fn data_ref(&self) -> Self::DataRef<'_> {
		self.data.data_ref()
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

impl<T: ?Sized, L: AsRef<T>> AsRef<T> for RetryingLockCollection<L> {
	fn as_ref(&self) -> &T {
		self.data.as_ref()
	}
}

impl<T: ?Sized, L: AsMut<T>> AsMut<T> for RetryingLockCollection<L> {
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
		// safety: the data cannot cannot contain references
		unsafe { Self::new_unchecked(data) }
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
		// safety: the data cannot cannot contain references
		unsafe { Self::new_unchecked(data) }
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
		// safety: the data is checked for duplicates before returning the collection
		(!contains_duplicates(&data)).then_some(unsafe { Self::new_unchecked(data) })
	}

	pub fn scoped_lock<'a, R>(&'a self, key: impl Keyable, f: impl Fn(L::DataMut<'a>) -> R) -> R {
		scoped_write(self, key, f)
	}

	pub fn scoped_try_lock<'a, Key: Keyable, R>(
		&'a self,
		key: Key,
		f: impl Fn(L::DataMut<'a>) -> R,
	) -> Result<R, Key> {
		scoped_try_write(self, key, f)
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
	pub fn lock(&self, key: ThreadKey) -> LockGuard<L::Guard<'_>> {
		unsafe {
			// safety: we're taking the thread key
			self.raw_write();

			LockGuard {
				// safety: we just locked the collection
				guard: self.guard(),
				key,
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
	pub fn try_lock(&self, key: ThreadKey) -> Result<LockGuard<L::Guard<'_>>, ThreadKey> {
		unsafe {
			// safety: we're taking the thread key
			if self.raw_try_write() {
				Ok(LockGuard {
					// safety: we just succeeded in locking everything
					guard: self.guard(),
					key,
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
	pub fn unlock(guard: LockGuard<L::Guard<'_>>) -> ThreadKey {
		drop(guard.guard);
		guard.key
	}
}

impl<L: Sharable> RetryingLockCollection<L> {
	pub fn scoped_read<'a, R>(&'a self, key: impl Keyable, f: impl Fn(L::DataRef<'a>) -> R) -> R {
		scoped_read(self, key, f)
	}

	pub fn scoped_try_read<'a, Key: Keyable, R>(
		&'a self,
		key: Key,
		f: impl Fn(L::DataRef<'a>) -> R,
	) -> Result<R, Key> {
		scoped_try_read(self, key, f)
	}

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
	pub fn read(&self, key: ThreadKey) -> LockGuard<L::ReadGuard<'_>> {
		unsafe {
			// safety: we're taking the thread key
			self.raw_read();

			LockGuard {
				// safety: we just locked the collection
				guard: self.read_guard(),
				key,
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
	///     Ok(mut guard) => {
	///         assert_eq!(*guard.0, 5);
	///         assert_eq!(*guard.1, "6");
	///     },
	///     Err(_) => unreachable!(),
	/// };
	///
	/// ```
	pub fn try_read(&self, key: ThreadKey) -> Result<LockGuard<L::ReadGuard<'_>>, ThreadKey> {
		unsafe {
			// safety: we're taking the thread key
			if !self.raw_try_read() {
				return Err(key);
			}

			Ok(LockGuard {
				// safety: we just succeeded in locking everything
				guard: self.read_guard(),
				key,
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
	pub fn unlock_read(guard: LockGuard<L::ReadGuard<'_>>) -> ThreadKey {
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
	#[allow(clippy::float_cmp)]
	fn uses_correct_default() {
		let collection =
			RetryingLockCollection::<(RwLock<f64>, Mutex<Option<i32>>, Mutex<usize>)>::default();
		let tuple = collection.into_inner();
		assert_eq!(tuple.0, 0.0);
		assert!(tuple.1.is_none());
		assert_eq!(tuple.2, 0)
	}

	#[test]
	fn from() {
		let key = ThreadKey::get().unwrap();
		let collection =
			RetryingLockCollection::from([Mutex::new("foo"), Mutex::new("bar"), Mutex::new("baz")]);
		let guard = collection.lock(key);
		assert_eq!(*guard[0], "foo");
		assert_eq!(*guard[1], "bar");
		assert_eq!(*guard[2], "baz");
	}

	#[test]
	fn new_ref_works() {
		let key = ThreadKey::get().unwrap();
		let mutexes = [Mutex::new(0), Mutex::new(1)];
		let collection = RetryingLockCollection::new_ref(&mutexes);
		collection.scoped_lock(key, |guard| {
			assert_eq!(*guard[0], 0);
			assert_eq!(*guard[1], 1);
		})
	}

	#[test]
	fn scoped_read_sees_changes() {
		let mut key = ThreadKey::get().unwrap();
		let mutexes = [RwLock::new(24), RwLock::new(42)];
		let collection = RetryingLockCollection::new(mutexes);
		collection.scoped_lock(&mut key, |guard| *guard[0] = 128);

		let sum = collection.scoped_read(&mut key, |guard| {
			assert_eq!(*guard[0], 128);
			assert_eq!(*guard[1], 42);
			*guard[0] + *guard[1]
		});

		assert_eq!(sum, 128 + 42);
	}

	#[test]
	fn get_mut_affects_scoped_read() {
		let mut key = ThreadKey::get().unwrap();
		let mutexes = [RwLock::new(24), RwLock::new(42)];
		let mut collection = RetryingLockCollection::new(mutexes);
		let guard = collection.get_mut();
		*guard[0] = 128;

		let sum = collection.scoped_read(&mut key, |guard| {
			assert_eq!(*guard[0], 128);
			assert_eq!(*guard[1], 42);
			*guard[0] + *guard[1]
		});

		assert_eq!(sum, 128 + 42);
	}

	#[test]
	fn scoped_try_lock_can_fail() {
		let key = ThreadKey::get().unwrap();
		let collection = RetryingLockCollection::new([Mutex::new(1), Mutex::new(2)]);
		let guard = collection.lock(key);

		std::thread::scope(|s| {
			s.spawn(|| {
				let key = ThreadKey::get().unwrap();
				let r = collection.scoped_try_lock(key, |_| {});
				assert!(r.is_err());
			});
		});

		drop(guard);
	}

	#[test]
	fn scoped_try_read_can_fail() {
		let key = ThreadKey::get().unwrap();
		let collection = RetryingLockCollection::new([RwLock::new(1), RwLock::new(2)]);
		let guard = collection.lock(key);

		std::thread::scope(|s| {
			s.spawn(|| {
				let key = ThreadKey::get().unwrap();
				let r = collection.scoped_try_read(key, |_| {});
				assert!(r.is_err());
			});
		});

		drop(guard);
	}

	#[test]
	fn try_lock_works() {
		let key = ThreadKey::get().unwrap();
		let collection = RetryingLockCollection::new([Mutex::new(1), Mutex::new(2)]);
		let guard = collection.try_lock(key);

		std::thread::scope(|s| {
			s.spawn(|| {
				let key = ThreadKey::get().unwrap();
				let guard = collection.try_lock(key);
				assert!(guard.is_err());
			});
		});

		assert!(guard.is_ok());
	}

	#[test]
	fn try_read_works() {
		let key = ThreadKey::get().unwrap();
		let collection = RetryingLockCollection::new([RwLock::new(1), RwLock::new(2)]);
		let guard = collection.try_read(key);

		std::thread::scope(|s| {
			s.spawn(|| {
				let key = ThreadKey::get().unwrap();
				let guard = collection.try_read(key);
				assert!(guard.is_ok());
			});
		});

		assert!(guard.is_ok());
	}

	#[test]
	fn try_read_fails_for_locked_collection() {
		let key = ThreadKey::get().unwrap();
		let mutexes = [RwLock::new(24), RwLock::new(42)];
		let collection = RetryingLockCollection::new_ref(&mutexes);

		std::thread::scope(|s| {
			s.spawn(|| {
				let key = ThreadKey::get().unwrap();
				let guard = mutexes[1].write(key);
				assert_eq!(*guard, 42);
				std::mem::forget(guard);
			});
		});

		let guard = collection.try_read(key);
		assert!(guard.is_err());
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
	fn from_iterator() {
		let key = ThreadKey::get().unwrap();
		let collection: RetryingLockCollection<Vec<Mutex<&str>>> =
			[Mutex::new("foo"), Mutex::new("bar"), Mutex::new("baz")]
				.into_iter()
				.collect();
		let guard = collection.lock(key);
		assert_eq!(*guard[0], "foo");
		assert_eq!(*guard[1], "bar");
		assert_eq!(*guard[2], "baz");
	}

	#[test]
	fn into_owned_iterator() {
		let collection = RetryingLockCollection::new([Mutex::new(0), Mutex::new(1), Mutex::new(2)]);
		for (i, mutex) in collection.into_iter().enumerate() {
			assert_eq!(mutex.into_inner(), i);
		}
	}

	#[test]
	fn into_ref_iterator() {
		let mut key = ThreadKey::get().unwrap();
		let collection = RetryingLockCollection::new([Mutex::new(0), Mutex::new(1), Mutex::new(2)]);
		for (i, mutex) in (&collection).into_iter().enumerate() {
			mutex.scoped_lock(&mut key, |val| assert_eq!(*val, i))
		}
	}

	#[test]
	fn ref_iterator() {
		let mut key = ThreadKey::get().unwrap();
		let collection = RetryingLockCollection::new([Mutex::new(0), Mutex::new(1), Mutex::new(2)]);
		for (i, mutex) in collection.iter().enumerate() {
			mutex.scoped_lock(&mut key, |val| assert_eq!(*val, i))
		}
	}

	#[test]
	fn mut_iterator() {
		let mut key = ThreadKey::get().unwrap();
		let mut collection =
			RetryingLockCollection::new([Mutex::new(0), Mutex::new(1), Mutex::new(2)]);
		for (i, mutex) in collection.iter_mut().enumerate() {
			mutex.scoped_lock(&mut key, |val| assert_eq!(*val, i))
		}
	}

	#[test]
	fn extend_collection() {
		let mutex1 = Mutex::new(0);
		let mutex2 = Mutex::new(0);
		let mut collection = RetryingLockCollection::new(vec![mutex1]);

		collection.extend([mutex2]);

		assert_eq!(collection.into_inner().len(), 2);
	}

	#[test]
	fn lock_empty_lock_collection() {
		let key = ThreadKey::get().unwrap();
		let collection: RetryingLockCollection<[RwLock<i32>; 0]> = RetryingLockCollection::new([]);

		let guard = collection.lock(key);
		assert!(guard.len() == 0);
		let key = RetryingLockCollection::<[RwLock<_>; 0]>::unlock(guard);

		let guard = collection.read(key);
		assert!(guard.len() == 0);
	}

	#[test]
	fn read_empty_lock_collection() {
		let key = ThreadKey::get().unwrap();
		let collection: RetryingLockCollection<[RwLock<i32>; 0]> = RetryingLockCollection::new([]);

		let guard = collection.read(key);
		assert!(guard.len() == 0);
		let key = RetryingLockCollection::<[RwLock<_>; 0]>::unlock_read(guard);

		let guard = collection.lock(key);
		assert!(guard.len() == 0);
	}

	#[test]
	fn as_ref_works() {
		let mutexes = [Mutex::new(0), Mutex::new(1)];
		let collection = RetryingLockCollection::new_ref(&mutexes);

		assert!(std::ptr::addr_eq(&mutexes, collection.as_ref()))
	}

	#[test]
	fn as_mut_works() {
		let mut mutexes = [Mutex::new(0), Mutex::new(1)];
		let mut collection = RetryingLockCollection::new(&mut mutexes);

		collection.as_mut()[0] = Mutex::new(42);

		assert_eq!(*collection.as_mut()[0].get_mut(), 42);
	}

	#[test]
	fn child() {
		let mutexes = [Mutex::new(0), Mutex::new(1)];
		let collection = RetryingLockCollection::new_ref(&mutexes);

		assert!(std::ptr::addr_eq(&mutexes, *collection.child()))
	}

	#[test]
	fn child_mut_works() {
		let mut mutexes = [Mutex::new(0), Mutex::new(1)];
		let mut collection = RetryingLockCollection::new(&mut mutexes);

		collection.child_mut()[0] = Mutex::new(42);

		assert_eq!(*collection.child_mut()[0].get_mut(), 42);
	}

	#[test]
	fn into_child_works() {
		let mutexes = [Mutex::new(0), Mutex::new(1)];
		let mut collection = RetryingLockCollection::new(mutexes);

		collection.child_mut()[0] = Mutex::new(42);

		assert_eq!(
			*collection
				.into_child()
				.as_mut()
				.get_mut(0)
				.unwrap()
				.get_mut(),
			42
		);
	}
}
