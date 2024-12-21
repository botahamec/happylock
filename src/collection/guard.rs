use std::fmt::{Debug, Display};
use std::ops::{Deref, DerefMut};

use crate::key::Keyable;

use super::LockGuard;

impl<Guard: Debug, Key: Keyable> Debug for LockGuard<'_, Guard, Key> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<Guard: Display, Key: Keyable> Display for LockGuard<'_, Guard, Key> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}

impl<Guard, Key: Keyable> Deref for LockGuard<'_, Guard, Key> {
	type Target = Guard;

	fn deref(&self) -> &Self::Target {
		&self.guard
	}
}

impl<Guard, Key: Keyable> DerefMut for LockGuard<'_, Guard, Key> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.guard
	}
}

impl<Guard, Key: Keyable> AsRef<Guard> for LockGuard<'_, Guard, Key> {
	fn as_ref(&self) -> &Guard {
		&self.guard
	}
}

impl<Guard, Key: Keyable> AsMut<Guard> for LockGuard<'_, Guard, Key> {
	fn as_mut(&mut self) -> &mut Guard {
		&mut self.guard
	}
}

#[cfg(test)]
mod tests {
	use crate::collection::OwnedLockCollection;
	use crate::{RwLock, ThreadKey};

	#[test]
	fn guard_display_works() {
		let key = ThreadKey::get().unwrap();
		let lock = OwnedLockCollection::new(RwLock::new("Hello, world!"));
		let guard = lock.read(key);
		assert_eq!(guard.to_string(), "Hello, world!".to_string());
	}
}
