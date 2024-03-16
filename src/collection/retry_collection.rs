use std::marker::PhantomData;

use crate::{lockable::Lock, Keyable, Lockable, OwnedLockable};

use super::{LockGuard, RetryingLockCollection};

fn contains_duplicates<L: Lockable>(data: L) -> bool {
	let mut locks = Vec::new();
	data.get_ptrs(&mut locks);
	let mut locks: Vec<_> = locks.into_iter().map(|l| l as *const dyn Lock).collect();
	locks.sort_unstable();
	locks.windows(2).any(|w| std::ptr::addr_eq(w[0], w[1]))
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

	pub fn lock<'a, 'key: 'a, Key: Keyable + 'key>(
		&'a self,
		key: Key,
	) -> LockGuard<'a, 'key, L, Key> {
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

	pub fn try_lock<'a, 'key: 'a, Key: Keyable + 'key>(
		&'a self,
		key: Key,
	) -> Option<LockGuard<'a, 'key, L, Key>> {
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

	pub fn unlock<'key, Key: Keyable + 'key>(guard: LockGuard<'_, 'key, L, Key>) -> Key {
		drop(guard.guard);
		guard.key
	}
}
