use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::ops::{Deref, DerefMut};

use crate::key::Keyable;

use super::LockGuard;

#[mutants::skip] // it's hard to get two guards safely
#[cfg(not(tarpaulin_include))]
impl<Guard: PartialEq, Key: Keyable> PartialEq for LockGuard<'_, Guard, Key> {
	fn eq(&self, other: &Self) -> bool {
		self.guard.eq(&other.guard)
	}
}

#[mutants::skip] // it's hard to get two guards safely
#[cfg(not(tarpaulin_include))]
impl<Guard: PartialOrd, Key: Keyable> PartialOrd for LockGuard<'_, Guard, Key> {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		self.guard.partial_cmp(&other.guard)
	}
}

#[mutants::skip] // it's hard to get two guards safely
#[cfg(not(tarpaulin_include))]
impl<Guard: Eq, Key: Keyable> Eq for LockGuard<'_, Guard, Key> {}

#[mutants::skip] // it's hard to get two guards safely
#[cfg(not(tarpaulin_include))]
impl<Guard: Ord, Key: Keyable> Ord for LockGuard<'_, Guard, Key> {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.guard.cmp(&other.guard)
	}
}

#[mutants::skip] // hashing involves RNG and is hard to test
#[cfg(not(tarpaulin_include))]
impl<Guard: Hash, Key: Keyable> Hash for LockGuard<'_, Guard, Key> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.guard.hash(state)
	}
}

#[mutants::skip]
#[cfg(not(tarpaulin_include))]
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
	use crate::{LockCollection, Mutex, RwLock, ThreadKey};

	#[test]
	fn guard_display_works() {
		let key = ThreadKey::get().unwrap();
		let lock = OwnedLockCollection::new(RwLock::new("Hello, world!"));
		let guard = lock.read(key);
		assert_eq!(guard.to_string(), "Hello, world!".to_string());
	}

	#[test]
	fn deref_mut_works() {
		let mut key = ThreadKey::get().unwrap();
		let locks = (Mutex::new(1), Mutex::new(2));
		let lock = LockCollection::new_ref(&locks);
		let mut guard = lock.lock(&mut key);
		*guard.0 = 3;
		drop(guard);

		let guard = locks.0.lock(&mut key);
		assert_eq!(*guard, 3);
		drop(guard);

		let guard = locks.1.lock(&mut key);
		assert_eq!(*guard, 2);
		drop(guard);
	}

	#[test]
	fn as_ref_works() {
		let mut key = ThreadKey::get().unwrap();
		let locks = (Mutex::new(1), Mutex::new(2));
		let lock = LockCollection::new_ref(&locks);
		let mut guard = lock.lock(&mut key);
		*guard.0 = 3;
		drop(guard);

		let guard = locks.0.lock(&mut key);
		assert_eq!(guard.as_ref(), &3);
		drop(guard);

		let guard = locks.1.lock(&mut key);
		assert_eq!(guard.as_ref(), &2);
		drop(guard);
	}

	#[test]
	fn as_mut_works() {
		let mut key = ThreadKey::get().unwrap();
		let locks = (Mutex::new(1), Mutex::new(2));
		let lock = LockCollection::new_ref(&locks);
		let mut guard = lock.lock(&mut key);
		let guard_mut = guard.as_mut();
		*guard_mut.0 = 3;
		drop(guard);

		let guard = locks.0.lock(&mut key);
		assert_eq!(guard.as_ref(), &3);
		drop(guard);

		let guard = locks.1.lock(&mut key);
		assert_eq!(guard.as_ref(), &2);
		drop(guard);
	}
}
