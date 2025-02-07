use std::cell::UnsafeCell;
use std::fmt::Debug;
use std::marker::PhantomData;

use crate::lockable::{Lockable, LockableIntoInner, OwnedLockable, RawLock, Sharable};
use crate::Keyable;

use super::{utils, BoxedLockCollection, LockGuard};

/// returns `true` if the sorted list contains a duplicate
#[must_use]
fn contains_duplicates(l: &[&dyn RawLock]) -> bool {
	if l.is_empty() {
		// Return early to prevent panic in the below call to `windows`
		return false;
	}

	l.windows(2)
		// NOTE: addr_eq is necessary because eq would also compare the v-table pointers
		.any(|window| std::ptr::addr_eq(window[0], window[1]))
}

unsafe impl<L: Lockable> RawLock for BoxedLockCollection<L> {
	#[mutants::skip] // this should never be called
	#[cfg(not(tarpaulin_include))]
	fn poison(&self) {
		for lock in &self.locks {
			lock.poison();
		}
	}

	unsafe fn raw_lock(&self) {
		utils::ordered_lock(self.locks())
	}

	unsafe fn raw_try_lock(&self) -> bool {
		println!("{}", self.locks().len());
		utils::ordered_try_lock(self.locks())
	}

	unsafe fn raw_unlock(&self) {
		for lock in self.locks() {
			lock.raw_unlock();
		}
	}

	unsafe fn raw_read(&self) {
		utils::ordered_read(self.locks());
	}

	unsafe fn raw_try_read(&self) -> bool {
		utils::ordered_try_read(self.locks())
	}

	unsafe fn raw_unlock_read(&self) {
		for lock in self.locks() {
			lock.raw_unlock_read();
		}
	}
}

unsafe impl<L: Lockable> Lockable for BoxedLockCollection<L> {
	type Guard<'g>
		= L::Guard<'g>
	where
		Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		ptrs.extend(self.locks())
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		self.child().guard()
	}
}

unsafe impl<L: Sharable> Sharable for BoxedLockCollection<L> {
	type ReadGuard<'g>
		= L::ReadGuard<'g>
	where
		Self: 'g;

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		self.child().read_guard()
	}
}

unsafe impl<L: OwnedLockable> OwnedLockable for BoxedLockCollection<L> {}

// LockableGetMut can't be implemented because that would create mutable and
// immutable references to the same value at the same time.

impl<L: LockableIntoInner> LockableIntoInner for BoxedLockCollection<L> {
	type Inner = L::Inner;

	fn into_inner(self) -> Self::Inner {
		LockableIntoInner::into_inner(self.into_child())
	}
}

impl<L> IntoIterator for BoxedLockCollection<L>
where
	L: IntoIterator,
{
	type Item = <L as IntoIterator>::Item;
	type IntoIter = <L as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.into_child().into_iter()
	}
}

impl<'a, L> IntoIterator for &'a BoxedLockCollection<L>
where
	&'a L: IntoIterator,
{
	type Item = <&'a L as IntoIterator>::Item;
	type IntoIter = <&'a L as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.child().into_iter()
	}
}

impl<L: OwnedLockable, I: FromIterator<L> + OwnedLockable> FromIterator<L>
	for BoxedLockCollection<I>
{
	fn from_iter<T: IntoIterator<Item = L>>(iter: T) -> Self {
		let iter: I = iter.into_iter().collect();
		Self::new(iter)
	}
}

// safety: the RawLocks must be send because they come from the Send Lockable
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<L: Send> Send for BoxedLockCollection<L> {}
unsafe impl<L: Sync> Sync for BoxedLockCollection<L> {}

impl<L> Drop for BoxedLockCollection<L> {
	#[mutants::skip] // i can't test for a memory leak
	#[cfg(not(tarpaulin_include))]
	fn drop(&mut self) {
		unsafe {
			// safety: this collection will never be locked again
			self.locks.clear();
			// safety: this was allocated using a box, and is now unique
			let boxed: Box<UnsafeCell<L>> = Box::from_raw(self.data.cast_mut());

			drop(boxed)
		}
	}
}

impl<T, L: AsRef<T>> AsRef<T> for BoxedLockCollection<L> {
	fn as_ref(&self) -> &T {
		self.child().as_ref()
	}
}

#[mutants::skip]
#[cfg(not(tarpaulin_include))]
impl<L: Debug> Debug for BoxedLockCollection<L> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct(stringify!(BoxedLockCollection))
			.field("data", &self.data)
			.finish_non_exhaustive()
	}
}

impl<L: OwnedLockable + Default> Default for BoxedLockCollection<L> {
	fn default() -> Self {
		Self::new(L::default())
	}
}

impl<L: OwnedLockable> From<L> for BoxedLockCollection<L> {
	fn from(value: L) -> Self {
		Self::new(value)
	}
}

impl<L> BoxedLockCollection<L> {
	/// Gets the underlying collection, consuming this collection.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey, LockCollection};
	///
	/// let data1 = Mutex::new(42);
	/// let data2 = Mutex::new("");
	///
	/// // data1 and data2 refer to distinct mutexes, so this won't panic
	/// let data = (&data1, &data2);
	/// let lock = LockCollection::try_new(&data).unwrap();
	///
	/// let key = ThreadKey::get().unwrap();
	/// let guard = lock.into_child().0.lock(key);
	/// assert_eq!(*guard, 42);
	/// ```
	#[must_use]
	pub fn into_child(mut self) -> L {
		unsafe {
			// safety: this collection will never be used again
			std::ptr::drop_in_place(&mut self.locks);
			// safety: this was allocated using a box, and is now unique
			let boxed: Box<UnsafeCell<L>> = Box::from_raw(self.data.cast_mut());
			// to prevent a double free
			std::mem::forget(self);

			boxed.into_inner()
		}
	}

	// child_mut is immediate UB because it leads to mutable and immutable
	// references happening at the same time

	/// Gets an immutable reference to the underlying data
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey, LockCollection};
	///
	/// let data1 = Mutex::new(42);
	/// let data2 = Mutex::new("");
	///
	/// // data1 and data2 refer to distinct mutexes, so this won't panic
	/// let data = (&data1, &data2);
	/// let lock = LockCollection::try_new(&data).unwrap();
	///
	/// let key = ThreadKey::get().unwrap();
	/// let guard = lock.child().0.lock(key);
	/// assert_eq!(*guard, 42);
	/// ```
	#[must_use]
	pub fn child(&self) -> &L {
		unsafe {
			self.data
				.as_ref()
				.unwrap_unchecked()
				.get()
				.as_ref()
				.unwrap_unchecked()
		}
	}

	/// Gets the locks
	fn locks(&self) -> &[&dyn RawLock] {
		&self.locks
	}
}

impl<L: OwnedLockable> BoxedLockCollection<L> {
	/// Creates a new collection of owned locks.
	///
	/// Because the locks are owned, there's no need to do any checks for
	/// duplicate values.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, LockCollection};
	///
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = LockCollection::new(data);
	/// ```
	#[must_use]
	pub fn new(data: L) -> Self {
		// safety: owned lockable types cannot contain duplicates
		unsafe { Self::new_unchecked(data) }
	}
}

impl<'a, L: OwnedLockable> BoxedLockCollection<&'a L> {
	/// Creates a new collection of owned locks.
	///
	/// Because the locks are owned, there's no need to do any checks for
	/// duplicate values.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, LockCollection};
	///
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = LockCollection::new_ref(&data);
	/// ```
	#[must_use]
	pub fn new_ref(data: &'a L) -> Self {
		// safety: owned lockable types cannot contain duplicates
		unsafe { Self::new_unchecked(data) }
	}
}

impl<L: Lockable> BoxedLockCollection<L> {
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
	/// use happylock::{Mutex, LockCollection};
	///
	/// let data1 = Mutex::new(0);
	/// let data2 = Mutex::new("");
	///
	/// // safety: data1 and data2 refer to distinct mutexes
	/// let data = (&data1, &data2);
	/// let lock = unsafe { LockCollection::new_unchecked(&data) };
	/// ```
	#[must_use]
	pub unsafe fn new_unchecked(data: L) -> Self {
		let data = Box::leak(Box::new(UnsafeCell::new(data)));
		let data_ref = data.get().cast_const().as_ref().unwrap_unchecked();

		let mut locks = Vec::new();
		data_ref.get_ptrs(&mut locks);

		// cast to *const () because fat pointers can't be converted to usize
		locks.sort_by_key(|lock| (&raw const **lock).cast::<()>() as usize);

		// safety we're just changing the lifetimes
		let locks: Vec<&'static dyn RawLock> = std::mem::transmute(locks);
		let data = &raw const *data;
		Self { data, locks }
	}

	/// Creates a new collection of locks.
	///
	/// This returns `None` if any locks are found twice in the given
	/// collection.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, LockCollection};
	///
	/// let data1 = Mutex::new(0);
	/// let data2 = Mutex::new("");
	///
	/// // data1 and data2 refer to distinct mutexes, so this won't panic
	/// let data = (&data1, &data2);
	/// let lock = LockCollection::try_new(&data).unwrap();
	/// ```
	#[must_use]
	pub fn try_new(data: L) -> Option<Self> {
		// safety: we are checking for duplicates before returning
		unsafe {
			let this = Self::new_unchecked(data);
			if contains_duplicates(this.locks()) {
				return None;
			}
			Some(this)
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
	/// use happylock::{Mutex, ThreadKey, LockCollection};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = LockCollection::new(data);
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
			// safety: we have the thread key
			self.raw_lock();

			LockGuard {
				// safety: we've already acquired the lock
				guard: self.child().guard(),
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
	/// If any locks in the collection are already locked, then an error
	/// containing the given key is returned.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey, LockCollection};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = LockCollection::new(data);
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
		let guard = unsafe {
			if !self.raw_try_lock() {
				return Err(key);
			}

			// safety: we've acquired the locks
			self.child().guard()
		};

		Ok(LockGuard {
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
	/// use happylock::{Mutex, ThreadKey, LockCollection};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = LockCollection::new(data);
	///
	/// let mut guard = lock.lock(key);
	/// *guard.0 += 1;
	/// *guard.1 = "1";
	/// let key = LockCollection::<(Mutex<i32>, Mutex<&str>)>::unlock(guard);
	/// ```
	pub fn unlock<'key, Key: Keyable + 'key>(guard: LockGuard<'key, L::Guard<'_>, Key>) -> Key {
		drop(guard.guard);
		guard.key
	}
}

impl<L: Sharable> BoxedLockCollection<L> {
	/// Locks the collection, so that other threads can still read from it
	///
	/// This function returns a guard that can be used to access the underlying
	/// data immutably. When the guard is dropped, the locks in the collection
	/// are also dropped.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey, LockCollection};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (RwLock::new(0), RwLock::new(""));
	/// let lock = LockCollection::new(data);
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
			// safety: we have the thread key
			self.raw_read();

			LockGuard {
				// safety: we've already acquired the lock
				guard: self.child().read_guard(),
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
	/// If any of the locks in the collection are already locked, then an error
	/// is returned containing the given key.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{RwLock, ThreadKey, LockCollection};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (RwLock::new(5), RwLock::new("6"));
	/// let lock = LockCollection::new(data);
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
	pub fn try_read<'g, 'key: 'g, Key: Keyable + 'key>(
		&'g self,
		key: Key,
	) -> Result<LockGuard<'key, L::ReadGuard<'g>, Key>, Key> {
		let guard = unsafe {
			// safety: we have the thread key
			if !self.raw_try_read() {
				return Err(key);
			}

			// safety: we've acquired the locks
			self.child().read_guard()
		};

		Ok(LockGuard {
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
	/// use happylock::{RwLock, ThreadKey, LockCollection};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (RwLock::new(0), RwLock::new(""));
	/// let lock = LockCollection::new(data);
	///
	/// let mut guard = lock.read(key);
	/// let key = LockCollection::<(RwLock<i32>, RwLock<&str>)>::unlock_read(guard);
	/// ```
	pub fn unlock_read<'key, Key: Keyable + 'key>(
		guard: LockGuard<'key, L::ReadGuard<'_>, Key>,
	) -> Key {
		drop(guard.guard);
		guard.key
	}
}

impl<L: LockableIntoInner> BoxedLockCollection<L> {
	/// Consumes this `BoxedLockCollection`, returning the underlying data.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, LockCollection};
	///
	/// let mutex = LockCollection::new([Mutex::new(0), Mutex::new(0)]);
	/// assert_eq!(mutex.into_inner(), [0, 0]);
	/// ```
	#[must_use]
	pub fn into_inner(self) -> <Self as LockableIntoInner>::Inner {
		LockableIntoInner::into_inner(self)
	}
}

impl<'a, L: 'a> BoxedLockCollection<L>
where
	&'a L: IntoIterator,
{
	/// Returns an iterator over references to each value in the collection.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey, LockCollection};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = [Mutex::new(26), Mutex::new(1)];
	/// let lock = LockCollection::new(data);
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
	fn from_iterator() {
		let key = ThreadKey::get().unwrap();
		let collection: BoxedLockCollection<Vec<Mutex<&str>>> =
			[Mutex::new("foo"), Mutex::new("bar"), Mutex::new("baz")]
				.into_iter()
				.collect();
		let guard = collection.lock(key);
		// TODO impl PartialEq<T> for MutexRef<T>
		assert_eq!(*guard[0], "foo");
		assert_eq!(*guard[1], "bar");
		assert_eq!(*guard[2], "baz");
	}

	#[test]
	fn from() {
		let key = ThreadKey::get().unwrap();
		let collection =
			BoxedLockCollection::from([Mutex::new("foo"), Mutex::new("bar"), Mutex::new("baz")]);
		let guard = collection.lock(key);
		// TODO impl PartialEq<T> for MutexRef<T>
		assert_eq!(*guard[0], "foo");
		assert_eq!(*guard[1], "bar");
		assert_eq!(*guard[2], "baz");
	}

	#[test]
	fn into_owned_iterator() {
		let collection = BoxedLockCollection::new([Mutex::new(0), Mutex::new(1), Mutex::new(2)]);
		for (i, mutex) in collection.into_iter().enumerate() {
			assert_eq!(mutex.into_inner(), i);
		}
	}

	#[test]
	fn into_ref_iterator() {
		let mut key = ThreadKey::get().unwrap();
		let collection = BoxedLockCollection::new([Mutex::new(0), Mutex::new(1), Mutex::new(2)]);
		for (i, mutex) in (&collection).into_iter().enumerate() {
			assert_eq!(*mutex.lock(&mut key), i);
		}
	}

	#[test]
	fn ref_iterator() {
		let mut key = ThreadKey::get().unwrap();
		let collection = BoxedLockCollection::new([Mutex::new(0), Mutex::new(1), Mutex::new(2)]);
		for (i, mutex) in collection.iter().enumerate() {
			assert_eq!(*mutex.lock(&mut key), i);
		}
	}

	#[test]
	#[allow(clippy::float_cmp)]
	fn uses_correct_default() {
		let collection =
			BoxedLockCollection::<(Mutex<f64>, Mutex<Option<i32>>, Mutex<usize>)>::default();
		let tuple = collection.into_inner();
		assert_eq!(tuple.0, 0.0);
		assert!(tuple.1.is_none());
		assert_eq!(tuple.2, 0)
	}

	#[test]
	fn non_duplicates_allowed() {
		let mutex1 = Mutex::new(0);
		let mutex2 = Mutex::new(1);
		assert!(BoxedLockCollection::try_new([&mutex1, &mutex2]).is_some())
	}

	#[test]
	fn duplicates_not_allowed() {
		let mutex1 = Mutex::new(0);
		assert!(BoxedLockCollection::try_new([&mutex1, &mutex1]).is_none())
	}

	#[test]
	fn contains_duplicates_empty() {
		assert!(!contains_duplicates(&[]))
	}

	#[test]
	fn try_lock_works() {
		let key = ThreadKey::get().unwrap();
		let collection = BoxedLockCollection::new([Mutex::new(1), Mutex::new(2)]);
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
		let collection = BoxedLockCollection::new([RwLock::new(1), RwLock::new(2)]);
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
	fn try_lock_fails_with_one_exclusive_lock() {
		let key = ThreadKey::get().unwrap();
		let locks = [Mutex::new(1), Mutex::new(2)];
		let collection = BoxedLockCollection::new_ref(&locks);
		let guard = locks[1].try_lock(key);

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
	fn try_read_fails_during_exclusive_lock() {
		let key = ThreadKey::get().unwrap();
		let collection = BoxedLockCollection::new([RwLock::new(1), RwLock::new(2)]);
		let guard = collection.try_lock(key);

		std::thread::scope(|s| {
			s.spawn(|| {
				let key = ThreadKey::get().unwrap();
				let guard = collection.try_read(key);
				assert!(guard.is_err());
			});
		});

		assert!(guard.is_ok());
	}

	#[test]
	fn try_read_fails_with_one_exclusive_lock() {
		let key = ThreadKey::get().unwrap();
		let locks = [RwLock::new(1), RwLock::new(2)];
		let collection = BoxedLockCollection::new_ref(&locks);
		let guard = locks[1].try_write(key);

		std::thread::scope(|s| {
			s.spawn(|| {
				let key = ThreadKey::get().unwrap();
				let guard = collection.try_read(key);
				assert!(guard.is_err());
			});
		});

		assert!(guard.is_ok());
	}

	#[test]
	fn unlock_collection_works() {
		let key = ThreadKey::get().unwrap();
		let mutex1 = Mutex::new("foo");
		let mutex2 = Mutex::new("bar");
		let collection = BoxedLockCollection::try_new((&mutex1, &mutex2)).unwrap();
		let guard = collection.lock(key);
		let key = BoxedLockCollection::<(&Mutex<_>, &Mutex<_>)>::unlock(guard);

		assert!(mutex1.try_lock(key).is_ok())
	}

	#[test]
	fn read_unlock_collection_works() {
		let key = ThreadKey::get().unwrap();
		let lock1 = RwLock::new("foo");
		let lock2 = RwLock::new("bar");
		let collection = BoxedLockCollection::try_new((&lock1, &lock2)).unwrap();
		let guard = collection.read(key);
		let key = BoxedLockCollection::<(&RwLock<_>, &RwLock<_>)>::unlock_read(guard);

		assert!(lock1.try_write(key).is_ok())
	}

	#[test]
	fn into_inner_works() {
		let collection = BoxedLockCollection::new((Mutex::new("Hello"), Mutex::new(47)));
		assert_eq!(collection.into_inner(), ("Hello", 47))
	}

	#[test]
	fn works_in_collection() {
		let key = ThreadKey::get().unwrap();
		let mutex1 = Mutex::new(0);
		let mutex2 = Mutex::new(1);
		let collection =
			BoxedLockCollection::try_new(BoxedLockCollection::try_new([&mutex1, &mutex2]).unwrap())
				.unwrap();

		let guard = collection.lock(key);
		assert!(mutex1.is_locked());
		assert!(mutex2.is_locked());
		drop(guard);
	}
}
