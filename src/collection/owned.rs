use crate::lockable::{
	Lockable, LockableGetMut, LockableIntoInner, OwnedLockable, RawLock, Sharable,
};
use crate::{Keyable, ThreadKey};

use super::{utils, LockGuard, OwnedLockCollection};

unsafe impl<L: Lockable> RawLock for OwnedLockCollection<L> {
	#[mutants::skip] // this should never run
	#[cfg(not(tarpaulin_include))]
	fn poison(&self) {
		let locks = utils::get_locks_unsorted(&self.data);
		for lock in locks {
			lock.poison();
		}
	}

	unsafe fn raw_lock(&self) {
		utils::ordered_lock(&utils::get_locks_unsorted(&self.data))
	}

	unsafe fn raw_try_lock(&self) -> bool {
		let locks = utils::get_locks_unsorted(&self.data);
		utils::ordered_try_lock(&locks)
	}

	unsafe fn raw_unlock(&self) {
		let locks = utils::get_locks_unsorted(&self.data);
		for lock in locks {
			lock.raw_unlock();
		}
	}

	unsafe fn raw_read(&self) {
		utils::ordered_read(&utils::get_locks_unsorted(&self.data))
	}

	unsafe fn raw_try_read(&self) -> bool {
		let locks = utils::get_locks_unsorted(&self.data);
		utils::ordered_try_read(&locks)
	}

	unsafe fn raw_unlock_read(&self) {
		let locks = utils::get_locks_unsorted(&self.data);
		for lock in locks {
			lock.raw_unlock_read();
		}
	}
}

unsafe impl<L: Lockable> Lockable for OwnedLockCollection<L> {
	type Guard<'g>
		= L::Guard<'g>
	where
		Self: 'g;

	type DataMut<'a>
		= L::DataMut<'a>
	where
		Self: 'a;

	#[mutants::skip] // It's hard to test lkocks in an OwnedLockCollection, because they're owned
	#[cfg(not(tarpaulin_include))]
	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		self.data.get_ptrs(ptrs)
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		self.data.guard()
	}

	unsafe fn data_mut(&self) -> Self::DataMut<'_> {
		self.data.data_mut()
	}
}

impl<L: LockableGetMut> LockableGetMut for OwnedLockCollection<L> {
	type Inner<'a>
		= L::Inner<'a>
	where
		Self: 'a;

	fn get_mut(&mut self) -> Self::Inner<'_> {
		self.data.get_mut()
	}
}

impl<L: LockableIntoInner> LockableIntoInner for OwnedLockCollection<L> {
	type Inner = L::Inner;

	fn into_inner(self) -> Self::Inner {
		self.data.into_inner()
	}
}

unsafe impl<L: Sharable> Sharable for OwnedLockCollection<L> {
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

unsafe impl<L: OwnedLockable> OwnedLockable for OwnedLockCollection<L> {}

impl<L> IntoIterator for OwnedLockCollection<L>
where
	L: IntoIterator,
{
	type Item = <L as IntoIterator>::Item;
	type IntoIter = <L as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

impl<L: OwnedLockable, I: FromIterator<L> + OwnedLockable> FromIterator<L>
	for OwnedLockCollection<I>
{
	fn from_iter<T: IntoIterator<Item = L>>(iter: T) -> Self {
		let iter: I = iter.into_iter().collect();
		Self::new(iter)
	}
}

impl<E: OwnedLockable + Extend<L>, L: OwnedLockable> Extend<L> for OwnedLockCollection<E> {
	fn extend<T: IntoIterator<Item = L>>(&mut self, iter: T) {
		self.data.extend(iter)
	}
}

// AsRef can't be implemented because an impl of AsRef<L> for L could break the
// invariant that there is only one way to lock the collection. AsMut is fine,
// because the collection can't be locked as long as the reference is valid.

impl<T, L: AsMut<T>> AsMut<T> for OwnedLockCollection<L> {
	fn as_mut(&mut self) -> &mut T {
		self.data.as_mut()
	}
}

impl<L: OwnedLockable + Default> Default for OwnedLockCollection<L> {
	fn default() -> Self {
		Self::new(L::default())
	}
}

impl<L: OwnedLockable> From<L> for OwnedLockCollection<L> {
	fn from(value: L) -> Self {
		Self::new(value)
	}
}

impl<L: OwnedLockable> OwnedLockCollection<L> {
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
	/// use happylock::collection::OwnedLockCollection;
	///
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = OwnedLockCollection::new(data);
	/// ```
	#[must_use]
	pub const fn new(data: L) -> Self {
		Self { data }
	}

	pub fn scoped_lock<R>(&self, key: impl Keyable, f: impl Fn(L::DataMut<'_>) -> R) -> R {
		unsafe {
			// safety: we have the thread key
			self.raw_lock();

			// safety: the data was just locked
			let r = f(self.data_mut());

			// safety: the collection is still locked
			self.raw_unlock();

			drop(key); // ensure the key stays alive long enough

			r
		}
	}

	pub fn scoped_try_lock<Key: Keyable, R>(
		&self,
		key: Key,
		f: impl Fn(L::DataMut<'_>) -> R,
	) -> Result<R, Key> {
		unsafe {
			// safety: we have the thread key
			if !self.raw_try_lock() {
				return Err(key);
			}

			// safety: we just locked the collection
			let r = f(self.data_mut());

			// safety: the collection is still locked
			self.raw_unlock();

			drop(key); // ensures the key stays valid long enough

			Ok(r)
		}
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
	/// use happylock::collection::OwnedLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = OwnedLockCollection::new(data);
	///
	/// let mut guard = lock.lock(key);
	/// *guard.0 += 1;
	/// *guard.1 = "1";
	/// ```
	pub fn lock(&self, key: ThreadKey) -> LockGuard<L::Guard<'_>> {
		let guard = unsafe {
			// safety: we have the thread key, and these locks happen in a
			//         predetermined order
			self.raw_lock();

			// safety: we've locked all of this already
			self.data.guard()
		};

		LockGuard { guard, key }
	}

	/// Attempts to lock the without blocking.
	///
	/// If the access could not be granted at this time, then `Err` is
	/// returned. Otherwise, an RAII guard is returned which will release the
	/// locks when it is dropped.
	///
	/// # Errors
	///
	/// If any of the locks in this collection are already locked, this returns
	/// an error containing the given key.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey};
	/// use happylock::collection::OwnedLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = OwnedLockCollection::new(data);
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
		let guard = unsafe {
			if !self.raw_try_lock() {
				return Err(key);
			}

			// safety: we've acquired the locks
			self.data.guard()
		};

		Ok(LockGuard { guard, key })
	}

	/// Unlocks the underlying lockable data type, returning the key that's
	/// associated with it.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey};
	/// use happylock::collection::OwnedLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = OwnedLockCollection::new(data);
	///
	/// let mut guard = lock.lock(key);
	/// *guard.0 += 1;
	/// *guard.1 = "1";
	/// let key = OwnedLockCollection::<(Mutex<i32>, Mutex<&str>)>::unlock(guard);
	/// ```
	#[allow(clippy::missing_const_for_fn)]
	pub fn unlock(guard: LockGuard<L::Guard<'_>>) -> ThreadKey {
		drop(guard.guard);
		guard.key
	}
}

impl<L: Sharable> OwnedLockCollection<L> {
	pub fn scoped_read<R>(&self, key: impl Keyable, f: impl Fn(L::DataRef<'_>) -> R) -> R {
		unsafe {
			// safety: we have the thread key
			self.raw_read();

			// safety: the data was just locked
			let r = f(self.data_ref());

			// safety: the collection is still locked
			self.raw_unlock_read();

			drop(key); // ensure the key stays alive long enough

			r
		}
	}

	pub fn scoped_try_read<Key: Keyable, R>(
		&self,
		key: Key,
		f: impl Fn(L::DataRef<'_>) -> R,
	) -> Result<R, Key> {
		unsafe {
			// safety: we have the thread key
			if !self.raw_try_read() {
				return Err(key);
			}

			// safety: we just locked the collection
			let r = f(self.data_ref());

			// safety: the collection is still locked
			self.raw_unlock_read();

			drop(key); // ensures the key stays valid long enough

			Ok(r)
		}
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
	/// use happylock::collection::OwnedLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (RwLock::new(0), RwLock::new(""));
	/// let lock = OwnedLockCollection::new(data);
	///
	/// let mut guard = lock.read(key);
	/// assert_eq!(*guard.0, 0);
	/// assert_eq!(*guard.1, "");
	/// ```
	pub fn read(&self, key: ThreadKey) -> LockGuard<L::ReadGuard<'_>> {
		unsafe {
			// safety: we have the thread key
			self.raw_read();

			LockGuard {
				// safety: we've already acquired the lock
				guard: self.data.read_guard(),
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
	/// If any of the locks in this collection can't be acquired, then an error
	/// is returned containing the given key.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey};
	/// use happylock::collection::OwnedLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (RwLock::new(5), RwLock::new("6"));
	/// let lock = OwnedLockCollection::new(data);
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
		let guard = unsafe {
			// safety: we have the thread key
			if !self.raw_try_read() {
				return Err(key);
			}

			// safety: we've acquired the locks
			self.data.read_guard()
		};

		Ok(LockGuard { guard, key })
	}

	/// Unlocks the underlying lockable data type, returning the key that's
	/// associated with it.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey};
	/// use happylock::collection::OwnedLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (RwLock::new(0), RwLock::new(""));
	/// let lock = OwnedLockCollection::new(data);
	///
	/// let mut guard = lock.read(key);
	/// let key = OwnedLockCollection::<(RwLock<i32>, RwLock<&str>)>::unlock_read(guard);
	/// ```
	#[allow(clippy::missing_const_for_fn)]
	pub fn unlock_read(guard: LockGuard<L::ReadGuard<'_>>) -> ThreadKey {
		drop(guard.guard);
		guard.key
	}
}

impl<L> OwnedLockCollection<L> {
	/// Gets the underlying collection, consuming this collection.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey};
	/// use happylock::collection::OwnedLockCollection;
	///
	/// let data = (Mutex::new(42), Mutex::new(""));
	/// let lock = OwnedLockCollection::new(data);
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

	/// Gets a mutable reference to the underlying collection.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey};
	/// use happylock::collection::OwnedLockCollection;
	///
	/// let data = (Mutex::new(42), Mutex::new(""));
	/// let mut lock = OwnedLockCollection::new(data);
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
}

impl<L: LockableGetMut> OwnedLockCollection<L> {
	/// Gets a mutable reference to the data behind this `OwnedLockCollection`.
	///
	/// Since this call borrows the `OwnedLockCollection` mutably, no actual
	/// locking needs to take place - the mutable borrow statically guarantees
	/// no locks exist.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, LockCollection};
	/// use happylock::collection::OwnedLockCollection;
	///
	/// let mut mutex = OwnedLockCollection::new([Mutex::new(0), Mutex::new(0)]);
	/// assert_eq!(mutex.get_mut(), [&mut 0, &mut 0]);
	/// ```
	pub fn get_mut(&mut self) -> L::Inner<'_> {
		LockableGetMut::get_mut(self)
	}
}

impl<L: LockableIntoInner> OwnedLockCollection<L> {
	/// Consumes this `OwnedLockCollection`, returning the underlying data.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, LockCollection};
	/// use happylock::collection::OwnedLockCollection;
	///
	/// let mutex = OwnedLockCollection::new([Mutex::new(0), Mutex::new(0)]);
	/// assert_eq!(mutex.into_inner(), [0, 0]);
	/// ```
	#[must_use]
	pub fn into_inner(self) -> L::Inner {
		LockableIntoInner::into_inner(self)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{Mutex, ThreadKey};

	#[test]
	fn get_mut_applies_changes() {
		let key = ThreadKey::get().unwrap();
		let mut collection = OwnedLockCollection::new([Mutex::new("foo"), Mutex::new("bar")]);
		assert_eq!(*collection.get_mut()[0], "foo");
		assert_eq!(*collection.get_mut()[1], "bar");
		*collection.get_mut()[0] = "baz";

		let guard = collection.lock(key);
		assert_eq!(*guard[0], "baz");
		assert_eq!(*guard[1], "bar");
	}

	#[test]
	fn into_inner_works() {
		let key = ThreadKey::get().unwrap();
		let collection = OwnedLockCollection::from([Mutex::new("foo")]);
		let mut guard = collection.lock(key);
		*guard[0] = "bar";
		drop(guard);

		let array = collection.into_inner();
		assert_eq!(array.len(), 1);
		assert_eq!(array[0], "bar");
	}

	#[test]
	fn from_into_iter_is_correct() {
		let array = [Mutex::new(0), Mutex::new(1), Mutex::new(2), Mutex::new(3)];
		let mut collection: OwnedLockCollection<Vec<Mutex<usize>>> = array.into_iter().collect();
		assert_eq!(collection.get_mut().len(), 4);
		for (i, lock) in collection.into_iter().enumerate() {
			assert_eq!(lock.into_inner(), i);
		}
	}

	#[test]
	fn from_iter_is_correct() {
		let array = [Mutex::new(0), Mutex::new(1), Mutex::new(2), Mutex::new(3)];
		let mut collection: OwnedLockCollection<Vec<Mutex<usize>>> = array.into_iter().collect();
		let collection: &mut Vec<_> = collection.as_mut();
		assert_eq!(collection.len(), 4);
		for (i, lock) in collection.iter_mut().enumerate() {
			assert_eq!(*lock.get_mut(), i);
		}
	}

	#[test]
	fn try_lock_works_on_unlocked() {
		let key = ThreadKey::get().unwrap();
		let collection = OwnedLockCollection::new((Mutex::new(0), Mutex::new(1)));
		let guard = collection.try_lock(key).unwrap();
		assert_eq!(*guard.0, 0);
		assert_eq!(*guard.1, 1);
	}

	#[test]
	fn try_lock_fails_on_locked() {
		let key = ThreadKey::get().unwrap();
		let collection = OwnedLockCollection::new((Mutex::new(0), Mutex::new(1)));

		std::thread::scope(|s| {
			s.spawn(|| {
				let key = ThreadKey::get().unwrap();
				#[allow(unused)]
				let guard = collection.lock(key);
				std::mem::forget(guard);
			});
		});

		assert!(collection.try_lock(key).is_err());
	}

	#[test]
	fn default_works() {
		type MyCollection = OwnedLockCollection<(Mutex<i32>, Mutex<Option<i32>>, Mutex<String>)>;
		let collection = MyCollection::default();
		let inner = collection.into_inner();
		assert_eq!(inner.0, 0);
		assert_eq!(inner.1, None);
		assert_eq!(inner.2, String::new());
	}

	#[test]
	fn can_be_extended() {
		let mutex1 = Mutex::new(0);
		let mutex2 = Mutex::new(1);
		let mut collection = OwnedLockCollection::new(vec![mutex1, mutex2]);

		collection.extend([Mutex::new(2)]);

		assert_eq!(collection.data.len(), 3);
	}
}
