use std::ops::{Deref, DerefMut};

use crate::{key::Keyable, Lockable};

use super::LockGuard;

impl<'a, 'key: 'a, L: Lockable<'a>, Key: Keyable> Deref for LockGuard<'a, 'key, L, Key> {
	type Target = L::Output;

	fn deref(&self) -> &Self::Target {
		&self.guard
	}
}

impl<'a, 'key: 'a, L: Lockable<'a>, Key: Keyable> DerefMut for LockGuard<'a, 'key, L, Key> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.guard
	}
}
