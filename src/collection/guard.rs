use std::ops::{Deref, DerefMut};

use crate::key::Keyable;

use super::LockGuard;

impl<'key, Guard, Key: Keyable> Deref for LockGuard<'key, Guard, Key> {
	type Target = Guard;

	fn deref(&self) -> &Self::Target {
		&self.guard
	}
}

impl<'key, Guard, Key: Keyable> DerefMut for LockGuard<'key, Guard, Key> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.guard
	}
}
