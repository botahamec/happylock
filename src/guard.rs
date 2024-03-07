use std::ops::{Deref, DerefMut};

use crate::{lockable::Lockable, ThreadKey};

pub struct LockGuard<'a, L: Lockable<'a>> {
	guard: L::Output,
	key: ThreadKey,
}

// TODO: docs
impl<'a, L: Lockable<'a>> LockGuard<'a, L> {
	pub fn lock(lock: &'a L, key: ThreadKey) -> Self {
		Self {
			// safety: we have the thread's key
			guard: unsafe { lock.lock() },
			key,
		}
	}

	pub fn try_lock(lock: &'a L, key: ThreadKey) -> Option<Self> {
		// safety: we have the thread's key
		unsafe { lock.try_lock() }.map(|guard| Self { guard, key })
	}

	#[allow(clippy::missing_const_for_fn)]
	pub fn unlock(self) -> ThreadKey {
		L::unlock(self.guard);
		self.key
	}
}

impl<'a, L: Lockable<'a>> Deref for LockGuard<'a, L> {
	type Target = L::Output;

	fn deref(&self) -> &Self::Target {
		&self.guard
	}
}

impl<'a, L: Lockable<'a>> DerefMut for LockGuard<'a, L> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.guard
	}
}
