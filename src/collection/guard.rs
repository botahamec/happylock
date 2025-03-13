use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::ops::{Deref, DerefMut};

use super::LockGuard;

#[mutants::skip] // hashing involves RNG and is hard to test
#[cfg(not(tarpaulin_include))]
impl<Guard: Hash> Hash for LockGuard<Guard> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.guard.hash(state)
	}
}

// No implementations of Eq, PartialEq, PartialOrd, or Ord
// You can't implement both PartialEq<Self> and PartialEq<T>
// It's easier to just implement neither and ask users to dereference
// This is less of a problem when using the scoped lock API

#[mutants::skip]
#[cfg(not(tarpaulin_include))]
impl<Guard: Debug> Debug for LockGuard<Guard> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<Guard: Display> Display for LockGuard<Guard> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}

impl<Guard> Deref for LockGuard<Guard> {
	type Target = Guard;

	fn deref(&self) -> &Self::Target {
		&self.guard
	}
}

impl<Guard> DerefMut for LockGuard<Guard> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.guard
	}
}

impl<Guard> AsRef<Guard> for LockGuard<Guard> {
	fn as_ref(&self) -> &Guard {
		&self.guard
	}
}

impl<Guard> AsMut<Guard> for LockGuard<Guard> {
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
		let key = ThreadKey::get().unwrap();
		let locks = (Mutex::new(1), Mutex::new(2));
		let lock = LockCollection::new_ref(&locks);
		let mut guard = lock.lock(key);
		*guard.0 = 3;
		let key = LockCollection::<(Mutex<_>, Mutex<_>)>::unlock(guard);

		let guard = locks.0.lock(key);
		assert_eq!(*guard, 3);
		let key = Mutex::unlock(guard);

		let guard = locks.1.lock(key);
		assert_eq!(*guard, 2);
	}

	#[test]
	fn as_ref_works() {
		let key = ThreadKey::get().unwrap();
		let locks = (Mutex::new(1), Mutex::new(2));
		let lock = LockCollection::new_ref(&locks);
		let mut guard = lock.lock(key);
		*guard.0 = 3;
		let key = LockCollection::<(Mutex<_>, Mutex<_>)>::unlock(guard);

		let guard = locks.0.lock(key);
		assert_eq!(guard.as_ref(), &3);
		let key = Mutex::unlock(guard);

		let guard = locks.1.lock(key);
		assert_eq!(guard.as_ref(), &2);
	}

	#[test]
	fn as_mut_works() {
		let key = ThreadKey::get().unwrap();
		let locks = (Mutex::new(1), Mutex::new(2));
		let lock = LockCollection::new_ref(&locks);
		let mut guard = lock.lock(key);
		let guard_mut = guard.as_mut();
		*guard_mut.0 = 3;
		let key = LockCollection::<(Mutex<_>, Mutex<_>)>::unlock(guard);

		let guard = locks.0.lock(key);
		assert_eq!(guard.as_ref(), &3);
		let key = Mutex::unlock(guard);

		let guard = locks.1.lock(key);
		assert_eq!(guard.as_ref(), &2);
	}
}
