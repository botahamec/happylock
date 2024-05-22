use crate::{lockable::Lock, Keyable, Lockable, OwnedLockable, Sharable};
use std::collections::HashSet;
use std::marker::PhantomData;

use super::{LockCollection, LockGuard, RetryingLockCollection};

fn contains_duplicates<L: Lockable>(data: L) -> bool {
	let mut locks = Vec::new();
	data.get_ptrs(&mut locks);
	let locks = locks.into_iter().map(|l| l as *const dyn Lock);

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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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

impl<L: Lockable> LockCollection<L> for RetryingLockCollection<L> {
	unsafe fn new_readonly(data: L) -> Self
	where
		L: Sharable,
	{
		// safety: this isn't being shared exclusively, so duplicates don't cause a deadlock
		unsafe { Self::new_unchecked(data) }
	}

	fn read<'g, 'key, Key: Keyable + 'key>(
		&self,
		key: Key,
	) -> LockGuard<'key, L::ReadGuard<'g>, Key>
	where
		L: Sharable,
	{
		self.read(key)
	}

	fn try_read<'g, 'key, Key: Keyable + 'key>(
		&self,
		key: Key,
	) -> Option<LockGuard<'key, L::ReadGuard<'g>, Key>>
	where
		L: Sharable,
	{
		self.try_read(key)
	}

	fn unlock_read<'key, Key: Keyable + 'key>(guard: LockGuard<'key, L::ReadGuard<'_>, Key>) -> Key
	where
		L: Sharable,
	{
		Self::unlock_read(guard)
	}
}
