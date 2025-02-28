use std::fmt::Debug;

use crate::lockable::{Lockable, OwnedLockable, RawLock, Sharable};
use crate::{Keyable, ThreadKey};

use super::utils::{get_locks, ordered_contains_duplicates};
use super::{utils, LockGuard, RefLockCollection};

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

unsafe impl<L: Lockable> RawLock for RefLockCollection<'_, L> {
	#[mutants::skip] // this should never run
	#[cfg(not(tarpaulin_include))]
	fn poison(&self) {
		for lock in &self.locks {
			lock.poison();
		}
	}

	unsafe fn raw_lock(&self) {
		utils::ordered_lock(&self.locks)
	}

	unsafe fn raw_try_lock(&self) -> bool {
		utils::ordered_try_lock(&self.locks)
	}

	unsafe fn raw_unlock(&self) {
		for lock in &self.locks {
			lock.raw_unlock();
		}
	}

	unsafe fn raw_read(&self) {
		utils::ordered_read(&self.locks)
	}

	unsafe fn raw_try_read(&self) -> bool {
		utils::ordered_try_read(&self.locks)
	}

	unsafe fn raw_unlock_read(&self) {
		for lock in &self.locks {
			lock.raw_unlock_read();
		}
	}
}

unsafe impl<L: Lockable> Lockable for RefLockCollection<'_, L> {
	type Guard<'g>
		= L::Guard<'g>
	where
		Self: 'g;

	type DataMut<'a>
		= L::DataMut<'a>
	where
		Self: 'a;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		ptrs.extend_from_slice(&self.locks);
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		self.data.guard()
	}

	unsafe fn data_mut(&self) -> Self::DataMut<'_> {
		self.data.data_mut()
	}
}

unsafe impl<L: Sharable> Sharable for RefLockCollection<'_, L> {
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

impl<T, L: AsRef<T>> AsRef<T> for RefLockCollection<'_, L> {
	fn as_ref(&self) -> &T {
		self.data.as_ref()
	}
}

#[mutants::skip]
#[cfg(not(tarpaulin_include))]
impl<L: Debug> Debug for RefLockCollection<'_, L> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct(stringify!(RefLockCollection))
			.field("data", self.data)
			.finish_non_exhaustive()
	}
}

// safety: the RawLocks must be send because they come from the Send Lockable
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<L: Send> Send for RefLockCollection<'_, L> {}
unsafe impl<L: Sync> Sync for RefLockCollection<'_, L> {}

impl<'a, L: OwnedLockable + Default> From<&'a L> for RefLockCollection<'a, L> {
	fn from(value: &'a L) -> Self {
		Self::new(value)
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
	/// use happylock::Mutex;
	/// use happylock::collection::RefLockCollection;
	///
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = RefLockCollection::new(&data);
	/// ```
	#[must_use]
	pub fn new(data: &'a L) -> Self {
		RefLockCollection {
			locks: get_locks(data),
			data,
		}
	}
}

impl<L> RefLockCollection<'_, L> {
	/// Gets an immutable reference to the underlying data
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey};
	/// use happylock::collection::RefLockCollection;
	///
	/// let data1 = Mutex::new(42);
	/// let data2 = Mutex::new("");
	///
	/// // data1 and data2 refer to distinct mutexes, so this won't panic
	/// let data = (&data1, &data2);
	/// let lock = RefLockCollection::try_new(&data).unwrap();
	///
	/// let key = ThreadKey::get().unwrap();
	/// let guard = lock.child().0.lock(key);
	/// assert_eq!(*guard, 42);
	/// ```
	#[must_use]
	pub const fn child(&self) -> &L {
		self.data
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
	/// use happylock::Mutex;
	/// use happylock::collection::RefLockCollection;
	///
	/// let data1 = Mutex::new(0);
	/// let data2 = Mutex::new("");
	///
	/// // safety: data1 and data2 refer to distinct mutexes
	/// let data = (&data1, &data2);
	/// let lock = unsafe { RefLockCollection::new_unchecked(&data) };
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
	/// use happylock::Mutex;
	/// use happylock::collection::RefLockCollection;
	///
	/// let data1 = Mutex::new(0);
	/// let data2 = Mutex::new("");
	///
	/// // data1 and data2 refer to distinct mutexes, so this won't panic
	/// let data = (&data1, &data2);
	/// let lock = RefLockCollection::try_new(&data).unwrap();
	/// ```
	#[must_use]
	pub fn try_new(data: &'a L) -> Option<Self> {
		let locks = get_locks(data);
		if ordered_contains_duplicates(&locks) {
			return None;
		}

		Some(Self { data, locks })
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
	/// use happylock::collection::RefLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = RefLockCollection::new(&data);
	///
	/// let mut guard = lock.lock(key);
	/// *guard.0 += 1;
	/// *guard.1 = "1";
	/// ```
	#[must_use]
	pub fn lock(&self, key: ThreadKey) -> LockGuard<L::Guard<'_>> {
		let guard = unsafe {
			// safety: we have the thread key
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
	/// If any of the locks in the collection are already locked, then an error
	/// is returned containing the given key.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey};
	/// use happylock::collection::RefLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = RefLockCollection::new(&data);
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
	/// use happylock::collection::RefLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = RefLockCollection::new(&data);
	///
	/// let mut guard = lock.lock(key);
	/// *guard.0 += 1;
	/// *guard.1 = "1";
	/// let key = RefLockCollection::<(Mutex<i32>, Mutex<&str>)>::unlock(guard);
	/// ```
	#[allow(clippy::missing_const_for_fn)]
	pub fn unlock(guard: LockGuard<L::Guard<'_>>) -> ThreadKey {
		drop(guard.guard);
		guard.key
	}
}

impl<L: Sharable> RefLockCollection<'_, L> {
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
	/// use happylock::collection::RefLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (RwLock::new(0), RwLock::new(""));
	/// let lock = RefLockCollection::new(&data);
	///
	/// let mut guard = lock.read(key);
	/// assert_eq!(*guard.0, 0);
	/// assert_eq!(*guard.1, "");
	/// ```
	#[must_use]
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
	/// If any of the locks in the collection are already locked, then an error
	/// is returned containing the given key.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey};
	/// use happylock::collection::RefLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (RwLock::new(5), RwLock::new("6"));
	/// let lock = RefLockCollection::new(&data);
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
	/// use happylock::collection::RefLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (RwLock::new(0), RwLock::new(""));
	/// let lock = RefLockCollection::new(&data);
	///
	/// let mut guard = lock.read(key);
	/// let key = RefLockCollection::<(RwLock<i32>, RwLock<&str>)>::unlock_read(guard);
	/// ```
	#[allow(clippy::missing_const_for_fn)]
	pub fn unlock_read(guard: LockGuard<L::ReadGuard<'_>>) -> ThreadKey {
		drop(guard.guard);
		guard.key
	}
}

impl<'a, L: 'a> RefLockCollection<'a, L>
where
	&'a L: IntoIterator,
{
	/// Returns an iterator over references to each value in the collection.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey};
	/// use happylock::collection::RefLockCollection;
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = [Mutex::new(26), Mutex::new(1)];
	/// let lock = RefLockCollection::new(&data);
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{Mutex, RwLock, ThreadKey};

	#[test]
	fn non_duplicates_allowed() {
		let mutex1 = Mutex::new(0);
		let mutex2 = Mutex::new(1);
		assert!(RefLockCollection::try_new(&[&mutex1, &mutex2]).is_some())
	}

	#[test]
	fn duplicates_not_allowed() {
		let mutex1 = Mutex::new(0);
		assert!(RefLockCollection::try_new(&[&mutex1, &mutex1]).is_none())
	}

	#[test]
	fn try_lock_succeeds_for_unlocked_collection() {
		let key = ThreadKey::get().unwrap();
		let mutexes = [Mutex::new(24), Mutex::new(42)];
		let collection = RefLockCollection::new(&mutexes);
		let guard = collection.try_lock(key).unwrap();
		assert_eq!(*guard[0], 24);
		assert_eq!(*guard[1], 42);
	}

	#[test]
	fn try_lock_fails_for_locked_collection() {
		let key = ThreadKey::get().unwrap();
		let mutexes = [Mutex::new(24), Mutex::new(42)];
		let collection = RefLockCollection::new(&mutexes);

		std::thread::scope(|s| {
			s.spawn(|| {
				let key = ThreadKey::get().unwrap();
				let guard = mutexes[1].lock(key);
				assert_eq!(*guard, 42);
				std::mem::forget(guard);
			});
		});

		let guard = collection.try_lock(key);
		assert!(guard.is_err());
	}

	#[test]
	fn try_read_succeeds_for_unlocked_collection() {
		let key = ThreadKey::get().unwrap();
		let mutexes = [RwLock::new(24), RwLock::new(42)];
		let collection = RefLockCollection::new(&mutexes);
		let guard = collection.try_read(key).unwrap();
		assert_eq!(*guard[0], 24);
		assert_eq!(*guard[1], 42);
	}

	#[test]
	fn try_read_fails_for_locked_collection() {
		let key = ThreadKey::get().unwrap();
		let mutexes = [RwLock::new(24), RwLock::new(42)];
		let collection = RefLockCollection::new(&mutexes);

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
	fn can_read_twice_on_different_threads() {
		let key = ThreadKey::get().unwrap();
		let mutexes = [RwLock::new(24), RwLock::new(42)];
		let collection = RefLockCollection::new(&mutexes);

		std::thread::scope(|s| {
			s.spawn(|| {
				let key = ThreadKey::get().unwrap();
				let guard = collection.read(key);
				assert_eq!(*guard[0], 24);
				assert_eq!(*guard[1], 42);
				std::mem::forget(guard);
			});
		});

		let guard = collection.try_read(key).unwrap();
		assert_eq!(*guard[0], 24);
		assert_eq!(*guard[1], 42);
	}

	#[test]
	fn works_in_collection() {
		let key = ThreadKey::get().unwrap();
		let mutex1 = Mutex::new(0);
		let mutex2 = Mutex::new(1);
		let collection0 = [&mutex1, &mutex2];
		let collection1 = RefLockCollection::try_new(&collection0).unwrap();
		let collection = RefLockCollection::try_new(&collection1).unwrap();

		let guard = collection.lock(key);
		assert!(mutex1.is_locked());
		assert!(mutex2.is_locked());
		drop(guard);
	}
}
