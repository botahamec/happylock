use std::fmt::{Debug, Display};
use std::ops::{Deref, DerefMut};

use crate::key::Keyable;

use super::LockGuard;

impl<'key, Guard: Debug, Key: Keyable> Debug for LockGuard<'key, Guard, Key> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<'key, Guard: Display, Key: Keyable> Display for LockGuard<'key, Guard, Key> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}

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

impl<'key, Guard, Key: Keyable> AsRef<Guard> for LockGuard<'key, Guard, Key> {
	fn as_ref(&self) -> &Guard {
		&self.guard
	}
}

impl<'key, Guard, Key: Keyable> AsMut<Guard> for LockGuard<'key, Guard, Key> {
	fn as_mut(&mut self) -> &mut Guard {
		&mut self.guard
	}
}
