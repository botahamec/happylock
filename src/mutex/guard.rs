use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use lock_api::RawMutex;

use crate::lockable::RawLock;
use crate::ThreadKey;

use super::{Mutex, MutexGuard, MutexRef};

// These impls make things slightly easier because now you can use
// `println!("{guard}")` instead of `println!("{}", *guard)`

#[mutants::skip] // hashing involves RNG and is hard to test
#[cfg(not(tarpaulin_include))]
impl<T: Hash + ?Sized, R: RawMutex> Hash for MutexRef<'_, T, R> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.deref().hash(state)
	}
}

#[mutants::skip]
#[cfg(not(tarpaulin_include))]
impl<T: Debug + ?Sized, R: RawMutex> Debug for MutexRef<'_, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<T: Display + ?Sized, R: RawMutex> Display for MutexRef<'_, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}

impl<T: ?Sized, R: RawMutex> Drop for MutexRef<'_, T, R> {
	fn drop(&mut self) {
		// safety: this guard is being destroyed, so the data cannot be
		//         accessed without locking again
		unsafe { self.0.raw_unlock_write() }
	}
}

impl<T: ?Sized, R: RawMutex> Deref for MutexRef<'_, T, R> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// safety: this is the only type that can use `value`, and there's
		//         a reference to this type, so there cannot be any mutable
		//         references to this value.
		unsafe { &*self.0.data.get() }
	}
}

impl<T: ?Sized, R: RawMutex> DerefMut for MutexRef<'_, T, R> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		// safety: this is the only type that can use `value`, and we have a
		//         mutable reference to this type, so there cannot be any other
		//         references to this value.
		unsafe { &mut *self.0.data.get() }
	}
}

impl<T: ?Sized, R: RawMutex> AsRef<T> for MutexRef<'_, T, R> {
	fn as_ref(&self) -> &T {
		self
	}
}

impl<T: ?Sized, R: RawMutex> AsMut<T> for MutexRef<'_, T, R> {
	fn as_mut(&mut self) -> &mut T {
		self
	}
}

impl<'a, T: ?Sized, R: RawMutex> MutexRef<'a, T, R> {
	/// Creates a reference to the underlying data of a mutex without
	/// attempting to lock it or take ownership of the key. But it's also quite
	/// dangerous to drop.
	pub(crate) const unsafe fn new(mutex: &'a Mutex<T, R>) -> Self {
		Self(mutex, PhantomData)
	}
}

// it's kinda annoying to re-implement some of this stuff on guards
// there's nothing i can do about that

#[mutants::skip] // hashing involves RNG and is hard to test
#[cfg(not(tarpaulin_include))]
impl<T: Hash + ?Sized, R: RawMutex> Hash for MutexGuard<'_, T, R> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.deref().hash(state)
	}
}

#[mutants::skip]
#[cfg(not(tarpaulin_include))]
impl<T: Debug + ?Sized, R: RawMutex> Debug for MutexGuard<'_, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<T: Display + ?Sized, R: RawMutex> Display for MutexGuard<'_, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}

impl<T: ?Sized, R: RawMutex> Deref for MutexGuard<'_, T, R> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.mutex
	}
}

impl<T: ?Sized, R: RawMutex> DerefMut for MutexGuard<'_, T, R> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.mutex
	}
}

impl<T: ?Sized, R: RawMutex> AsRef<T> for MutexGuard<'_, T, R> {
	fn as_ref(&self) -> &T {
		self
	}
}

impl<T: ?Sized, R: RawMutex> AsMut<T> for MutexGuard<'_, T, R> {
	fn as_mut(&mut self) -> &mut T {
		self
	}
}

impl<'a, T: ?Sized, R: RawMutex> MutexGuard<'a, T, R> {
	/// Create a guard to the given mutex. Undefined if multiple guards to the
	/// same mutex exist at once.
	#[must_use]
	pub(super) const unsafe fn new(mutex: &'a Mutex<T, R>, thread_key: ThreadKey) -> Self {
		Self {
			mutex: MutexRef(mutex, PhantomData),
			thread_key,
		}
	}
}

unsafe impl<T: ?Sized + Sync, R: RawMutex + Sync> Sync for MutexRef<'_, T, R> {}
