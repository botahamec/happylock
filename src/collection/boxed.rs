use std::fmt::Debug;
use std::marker::PhantomData;

use crate::lockable::{Lockable, OwnedLockable, RawLock, Sharable};
use crate::Keyable;

use super::{BoxedLockCollection, LockGuard};

/// returns `true` if the sorted list contains a duplicate
#[must_use]
fn contains_duplicates(l: &[&dyn RawLock]) -> bool {
	l.windows(2)
		.any(|window| std::ptr::eq(window[0], window[1]))
}

unsafe impl<L: Lockable> Lockable for BoxedLockCollection<L> {
	type Guard<'g> = L::Guard<'g> where Self: 'g;

	type ReadGuard<'g> = L::ReadGuard<'g> where Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		self.data.get_ptrs(ptrs)
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		self.data.guard()
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		self.data.read_guard()
	}
}

unsafe impl<L: Sharable> Sharable for BoxedLockCollection<L> {}

unsafe impl<L: OwnedLockable> OwnedLockable for BoxedLockCollection<L> {}

impl<L> IntoIterator for BoxedLockCollection<L>
where
	L: IntoIterator,
{
	type Item = <L as IntoIterator>::Item;
	type IntoIter = <L as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

impl<'a, L> IntoIterator for &'a BoxedLockCollection<L>
where
	&'a L: IntoIterator,
{
	type Item = <&'a L as IntoIterator>::Item;
	type IntoIter = <&'a L as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

impl<'a, L> IntoIterator for &'a mut BoxedLockCollection<L>
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
	for BoxedLockCollection<I>
{
	fn from_iter<T: IntoIterator<Item = L>>(iter: T) -> Self {
		let iter: I = iter.into_iter().collect();
		Self::new(iter)
	}
}

impl<E: OwnedLockable + Extend<L>, L: OwnedLockable> Extend<L> for BoxedLockCollection<E> {
	fn extend<T: IntoIterator<Item = L>>(&mut self, iter: T) {
		self.data.extend(iter)
	}
}

impl<L> AsRef<L> for BoxedLockCollection<L> {
	fn as_ref(&self) -> &L {
		&self.data
	}
}

impl<L> AsMut<L> for BoxedLockCollection<L> {
	fn as_mut(&mut self) -> &mut L {
		&mut self.data
	}
}

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

impl<L: OwnedLockable + Default> From<L> for BoxedLockCollection<L> {
	fn from(value: L) -> Self {
		Self::new(value)
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
		let data = Box::new(data);
		let mut locks = Vec::new();
		data.get_ptrs(&mut locks);
		locks.sort_by_key(|lock| std::ptr::from_ref(*lock).cast::<()>() as usize);

		// safety: the box will be dropped after the lock references, so it's
		//         safe to just pretend they're static
		let locks = std::mem::transmute(locks);
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
			if contains_duplicates(&this.locks) {
				return None;
			}
			Some(this)
		}
	}

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
	/// let guard = lock.into_inner().0.lock(key);
	/// assert_eq!(*guard, 42);
	/// ```
	#[must_use]
	pub fn into_inner(self) -> Box<L> {
		self.data
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
	/// use happylock::{Mutex, ThreadKey, LockCollection};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = (Mutex::new(0), Mutex::new(""));
	/// let lock = LockCollection::new(data);
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
	pub fn try_lock<'g, 'key: 'g, Key: Keyable + 'key>(
		&'g self,
		key: Key,
	) -> Option<LockGuard<'key, L::Guard<'g>, Key>> {
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
		for lock in &self.locks {
			// safety: we have the thread key
			unsafe { lock.read() };
		}

		LockGuard {
			// safety: we've already acquired the lock
			guard: unsafe { self.data.read_guard() },
			key,
			_phantom: PhantomData,
		}
	}

	/// Attempts to lock the without blocking, in such a way that other threads
	/// can still read from the collection.
	///
	/// If successful, this method returns a guard that can be used to access
	/// the data immutably, and unlocks the data when it is dropped. Otherwise,
	/// `None` is returned.
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
		let guard = unsafe {
			for (i, lock) in self.locks.iter().enumerate() {
				// safety: we have the thread key
				let success = lock.try_read();

				if !success {
					for lock in &self.locks[0..i] {
						// safety: this lock was already acquired
						lock.unlock_read();
					}
					return None;
				}
			}

			// safety: we've acquired the locks
			self.data.read_guard()
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

impl<'a, L: 'a> BoxedLockCollection<L>
where
	&'a mut L: IntoIterator,
{
	/// Returns an iterator over mutable references to each value in the
	/// collection.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::{Mutex, ThreadKey, LockCollection};
	///
	/// let key = ThreadKey::get().unwrap();
	/// let data = [Mutex::new(26), Mutex::new(1)];
	/// let mut lock = LockCollection::new(data);
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
