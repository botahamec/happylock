use std::marker::PhantomData;

use crate::{lockable::Lock, Keyable, Lockable, OwnedLockable};

use super::{LockGuard, OwnedLockCollection};

fn get_locks<L: Lockable>(data: &L) -> Vec<&dyn Lock> {
	let mut locks = Vec::new();
	data.get_ptrs(&mut locks);
	locks
}

impl<L: OwnedLockable> OwnedLockCollection<L> {
	#[must_use]
	pub const fn new(data: L) -> Self {
		Self { data }
	}

	pub fn lock<'a, 'key, Key: Keyable + 'key>(&'a self, key: Key) -> LockGuard<'a, 'key, L, Key> {
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

	pub fn try_lock<'a, 'key: 'a, Key: Keyable + 'key>(
		&'a self,
		key: Key,
	) -> Option<LockGuard<'a, 'key, L, Key>> {
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
}
