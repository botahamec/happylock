use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::Deref;

use lock_api::RawRwLock;

use crate::lockable::RawLock;
use crate::ThreadKey;

use super::{RwLock, RwLockReadGuard, RwLockReadRef};

// These impls make things slightly easier because now you can use
// `println!("{guard}")` instead of `println!("{}", *guard)`

#[mutants::skip] // hashing involves PRNG and is hard to test
#[cfg(not(tarpaulin_include))]
impl<T: Hash + ?Sized, R: RawRwLock> Hash for RwLockReadRef<'_, T, R> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.deref().hash(state)
	}
}

#[mutants::skip]
#[cfg(not(tarpaulin_include))]
impl<T: Debug + ?Sized, R: RawRwLock> Debug for RwLockReadRef<'_, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<T: Display + ?Sized, R: RawRwLock> Display for RwLockReadRef<'_, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}

impl<T: ?Sized, R: RawRwLock> Deref for RwLockReadRef<'_, T, R> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// safety: this is the only type that can use `value`, and there's
		//         a reference to this type, so there cannot be any mutable
		//         references to this value.
		unsafe { &*self.0.data.get() }
	}
}

impl<T: ?Sized, R: RawRwLock> AsRef<T> for RwLockReadRef<'_, T, R> {
	fn as_ref(&self) -> &T {
		self
	}
}

impl<T: ?Sized, R: RawRwLock> Drop for RwLockReadRef<'_, T, R> {
	fn drop(&mut self) {
		// safety: this guard is being destroyed, so the data cannot be
		//         accessed without locking again
		unsafe { self.0.raw_unlock_read() }
	}
}

impl<'a, T: ?Sized, R: RawRwLock> RwLockReadRef<'a, T, R> {
	/// Creates an immutable reference for the underlying data of an [`RwLock`]
	/// without locking it or taking ownership of the key.
	#[must_use]
	pub(crate) unsafe fn new(mutex: &'a RwLock<T, R>) -> Self {
		Self(mutex, PhantomData)
	}
}

#[mutants::skip] // hashing involves PRNG and is hard to test
#[cfg(not(tarpaulin_include))]
impl<T: Hash + ?Sized, R: RawRwLock> Hash for RwLockReadGuard<'_, T, R> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.deref().hash(state)
	}
}

#[mutants::skip]
#[cfg(not(tarpaulin_include))]
impl<T: Debug + ?Sized, R: RawRwLock> Debug for RwLockReadGuard<'_, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<T: Display + ?Sized, R: RawRwLock> Display for RwLockReadGuard<'_, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}

impl<T: ?Sized, R: RawRwLock> Deref for RwLockReadGuard<'_, T, R> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.rwlock
	}
}

impl<T: ?Sized, R: RawRwLock> AsRef<T> for RwLockReadGuard<'_, T, R> {
	fn as_ref(&self) -> &T {
		self
	}
}

impl<'a, T: ?Sized, R: RawRwLock> RwLockReadGuard<'a, T, R> {
	/// Create a guard to the given mutex. Undefined if multiple guards to the
	/// same mutex exist at once.
	#[must_use]
	pub(super) unsafe fn new(rwlock: &'a RwLock<T, R>, thread_key: ThreadKey) -> Self {
		Self {
			rwlock: RwLockReadRef(rwlock, PhantomData),
			thread_key,
		}
	}
}

unsafe impl<T: ?Sized + Sync, R: RawRwLock + Sync> Sync for RwLockReadRef<'_, T, R> {}
