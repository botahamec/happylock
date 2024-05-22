use std::marker::PhantomData;

use crate::{lockable::Lock, Keyable, Lockable, OwnedLockable, Sharable};

use super::{LockGuard, OwnedLockCollection};

fn get_locks<L: Lockable>(data: &L) -> Vec<&dyn Lock> {
	let mut locks = Vec::new();
	data.get_ptrs(&mut locks);
	locks
}

unsafe impl<L: Lockable> Lockable for OwnedLockCollection<L> {
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

unsafe impl<L: Sharable> Sharable for OwnedLockCollection<L> {}

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

impl<L: OwnedLockable> AsMut<L> for OwnedLockCollection<L> {
	fn as_mut(&mut self) -> &mut L {
		&mut self.data
	}
}

impl<L: OwnedLockable + Default> Default for OwnedLockCollection<L> {
	fn default() -> Self {
		Self::new(L::default())
	}
}

impl<L: OwnedLockable + Default> From<L> for OwnedLockCollection<L> {
	fn from(value: L) -> Self {
		Self::new(value)
	}
}

impl<L: OwnedLockable> OwnedLockCollection<L> {
	#[must_use]
	pub const fn new(data: L) -> Self {
		Self { data }
	}

	#[must_use]
	pub fn into_inner(self) -> L {
		self.data
	}

	pub fn lock<'g, 'key, Key: Keyable + 'key>(
		&'g self,
		key: Key,
	) -> LockGuard<'key, L::Guard<'g>, Key> {
		let locks = get_locks(&self.data);
		for lock in locks {
			// safety: we have the thread key, and these locks happen in a
			//         predetermined order
			unsafe { lock.lock() };
		}

		// safety: we've locked all of this already
		let guard = unsafe { self.data.guard() };
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
		let locks = get_locks(&self.data);
		let guard = unsafe {
			for (i, lock) in locks.iter().enumerate() {
				// safety: we have the thread key
				let success = lock.try_lock();

				if !success {
					for lock in &locks[0..i] {
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

	#[allow(clippy::missing_const_for_fn)]
	pub fn unlock<'g, 'key: 'g, Key: Keyable + 'key>(
		guard: LockGuard<'key, L::Guard<'g>, Key>,
	) -> Key {
		drop(guard.guard);
		guard.key
	}
}

impl<L: Sharable> OwnedLockCollection<L> {
	pub fn read<'g, 'key, Key: Keyable + 'key>(
		&'g self,
		key: Key,
	) -> LockGuard<'key, L::ReadGuard<'g>, Key> {
		let locks = get_locks(&self.data);
		for lock in locks {
			// safety: we have the thread key, and these locks happen in a
			//         predetermined order
			unsafe { lock.read() };
		}

		// safety: we've locked all of this already
		let guard = unsafe { self.data.read_guard() };
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
		let locks = get_locks(&self.data);
		let guard = unsafe {
			for (i, lock) in locks.iter().enumerate() {
				// safety: we have the thread key
				let success = lock.try_read();

				if !success {
					for lock in &locks[0..i] {
						// safety: this lock was already acquired
						lock.unlock();
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

	#[allow(clippy::missing_const_for_fn)]
	pub fn unlock_read<'g, 'key: 'g, Key: Keyable + 'key>(
		guard: LockGuard<'key, L::ReadGuard<'g>, Key>,
	) -> Key {
		drop(guard.guard);
		guard.key
	}
}
