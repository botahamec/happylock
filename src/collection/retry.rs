use crate::lockable::{Lockable, OwnedLockable, RawLock, Sharable};
use crate::Keyable;

use std::collections::HashSet;
use std::marker::PhantomData;

use super::{LockGuard, RetryingLockCollection};

fn contains_duplicates<L: Lockable>(data: L) -> bool {
	let mut locks = Vec::new();
	data.get_ptrs(&mut locks);
	let locks = locks.into_iter().map(|l| l as *const dyn RawLock);

	let mut locks_set = HashSet::new();
	for lock in locks {
		if !locks_set.insert(lock) {
			return true;
		}
	}

	false
}

unsafe impl<L: Lockable> Lockable for RetryingLockCollection<L> {
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

unsafe impl<L: Sharable> Sharable for RetryingLockCollection<L> {}

unsafe impl<L: OwnedLockable> OwnedLockable for RetryingLockCollection<L> {}

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

impl<L> AsRef<L> for RetryingLockCollection<L> {
	fn as_ref(&self) -> &L {
		&self.data
	}
}

impl<L> AsMut<L> for RetryingLockCollection<L> {
	fn as_mut(&mut self) -> &mut L {
		&mut self.data
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
	#[must_use]
	pub const fn new(data: L) -> Self {
		Self { data }
	}
}

impl<'a, L: OwnedLockable> RetryingLockCollection<&'a L> {
	#[must_use]
	pub const fn new_ref(data: &'a L) -> Self {
		Self { data }
	}
}

impl<L: Lockable> RetryingLockCollection<L> {
	#[must_use]
	pub const unsafe fn new_unchecked(data: L) -> Self {
		Self { data }
	}

	pub fn try_new(data: L) -> Option<Self> {
		contains_duplicates(&data).then_some(Self { data })
	}

	pub fn into_inner(self) -> L {
		self.data
	}

	pub fn lock<'g, 'key: 'g, Key: Keyable + 'key>(
		&'g self,
		key: Key,
	) -> LockGuard<'key, L::Guard<'g>, Key> {
		let mut first_index = 0;
		let mut locks = Vec::new();
		self.data.get_ptrs(&mut locks);

		if locks.is_empty() {
			return LockGuard {
				// safety: there's no data being returned
				guard: unsafe { self.data.guard() },
				key,
				_phantom: PhantomData,
			};
		}

		let guard = unsafe {
			'outer: loop {
				// safety: we have the thread key
				locks[first_index].lock();
				for (i, lock) in locks.iter().enumerate() {
					if i == first_index {
						continue;
					}

					// safety: we have the thread key
					if !lock.try_lock() {
						for lock in locks.iter().take(i) {
							// safety: we already locked all of these
							lock.unlock();
						}

						if first_index >= i {
							// safety: this is already locked and can't be unlocked
							//         by the previous loop
							locks[first_index].unlock();
						}

						first_index = i;
						continue 'outer;
					}
				}

				// safety: we locked all the data
				break self.data.guard();
			}
		};

		LockGuard {
			guard,
			key,
			_phantom: PhantomData,
		}
	}

	pub fn try_lock<'g, 'key: 'g, Key: Keyable + 'key>(
		&'g self,
		key: Key,
	) -> Option<LockGuard<'key, L::Guard<'g>, Key>> {
		let mut locks = Vec::new();
		self.data.get_ptrs(&mut locks);

		if locks.is_empty() {
			return Some(LockGuard {
				// safety: there's no data being returned
				guard: unsafe { self.data.guard() },
				key,
				_phantom: PhantomData,
			});
		}

		let guard = unsafe {
			for (i, lock) in locks.iter().enumerate() {
				// safety: we have the thread key
				if !lock.try_lock() {
					for lock in locks.iter().take(i) {
						// safety: we already locked all of these
						lock.unlock();
					}
					return None;
				}
			}

			// safety: we locked all the data
			self.data.guard()
		};

		Some(LockGuard {
			guard,
			key,
			_phantom: PhantomData,
		})
	}

	pub fn unlock<'key, Key: Keyable + 'key>(guard: LockGuard<'key, L::Guard<'_>, Key>) -> Key {
		drop(guard.guard);
		guard.key
	}
}

impl<L: Sharable> RetryingLockCollection<L> {
	pub fn read<'g, 'key: 'g, Key: Keyable + 'key>(
		&'g self,
		key: Key,
	) -> LockGuard<'key, L::ReadGuard<'g>, Key> {
		let mut first_index = 0;
		let mut locks = Vec::new();
		self.data.get_ptrs(&mut locks);

		if locks.is_empty() {
			return LockGuard {
				// safety: there's no data being returned
				guard: unsafe { self.data.read_guard() },
				key,
				_phantom: PhantomData,
			};
		}

		let guard = unsafe {
			'outer: loop {
				// safety: we have the thread key
				locks[first_index].read();
				for (i, lock) in locks.iter().enumerate() {
					if i == first_index {
						continue;
					}

					// safety: we have the thread key
					if !lock.try_read() {
						for lock in locks.iter().take(i) {
							// safety: we already locked all of these
							lock.unlock_read();
						}

						if first_index >= i {
							// safety: this is already locked and can't be unlocked
							//         by the previous loop
							locks[first_index].unlock_read();
						}

						first_index = i;
						continue 'outer;
					}
				}

				// safety: we locked all the data
				break self.data.read_guard();
			}
		};

		LockGuard {
			guard,
			key,
			_phantom: PhantomData,
		}
	}

	pub fn try_read<'g, 'key: 'g, Key: Keyable + 'key>(
		&'g self,
		key: Key,
	) -> Option<LockGuard<'key, L::ReadGuard<'g>, Key>> {
		let mut locks = Vec::new();
		self.data.get_ptrs(&mut locks);

		if locks.is_empty() {
			return Some(LockGuard {
				// safety: there's no data being returned
				guard: unsafe { self.data.read_guard() },
				key,
				_phantom: PhantomData,
			});
		}

		let guard = unsafe {
			for (i, lock) in locks.iter().enumerate() {
				// safety: we have the thread key
				if !lock.try_read() {
					for lock in locks.iter().take(i) {
						// safety: we already locked all of these
						lock.unlock_read();
					}
					return None;
				}
			}

			// safety: we locked all the data
			self.data.read_guard()
		};

		Some(LockGuard {
			guard,
			key,
			_phantom: PhantomData,
		})
	}

	pub fn unlock_read<'key, Key: Keyable + 'key>(
		guard: LockGuard<'key, L::ReadGuard<'_>, Key>,
	) -> Key {
		drop(guard.guard);
		guard.key
	}
}

impl<'a, L: 'a> RetryingLockCollection<L>
where
	&'a L: IntoIterator,
{
	/// Returns an iterator over references to each value in the collection.
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
	#[must_use]
	pub fn iter_mut(&'a mut self) -> <&'a mut L as IntoIterator>::IntoIter {
		self.into_iter()
	}
}
