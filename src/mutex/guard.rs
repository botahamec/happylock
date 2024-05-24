use std::fmt::{Debug, Display};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use lock_api::RawMutex;

use crate::key::Keyable;

use super::{Mutex, MutexGuard, MutexRef};

// This makes things slightly easier because now you can use
// `println!("{guard}")` instead of `println!("{}", *guard)`. I wonder if I
// should implement some other standard library traits like this too?
impl<'a, T: Debug + ?Sized + 'a, R: RawMutex> Debug for MutexRef<'a, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<'a, T: Display + ?Sized + 'a, R: RawMutex> Display for MutexRef<'a, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}

impl<'a, T: ?Sized + 'a, R: RawMutex> Drop for MutexRef<'a, T, R> {
	fn drop(&mut self) {
		// safety: this guard is being destroyed, so the data cannot be
		//         accessed without locking again
		unsafe { self.0.force_unlock() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawMutex> Deref for MutexRef<'a, T, R> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// safety: this is the only type that can use `value`, and there's
		//         a reference to this type, so there cannot be any mutable
		//         references to this value.
		unsafe { &*self.0.data.get() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawMutex> DerefMut for MutexRef<'a, T, R> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		// safety: this is the only type that can use `value`, and we have a
		//         mutable reference to this type, so there cannot be any other
		//         references to this value.
		unsafe { &mut *self.0.data.get() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawMutex> AsRef<T> for MutexRef<'a, T, R> {
	fn as_ref(&self) -> &T {
		self
	}
}

impl<'a, T: ?Sized + 'a, R: RawMutex> AsMut<T> for MutexRef<'a, T, R> {
	fn as_mut(&mut self) -> &mut T {
		self
	}
}

impl<'a, T: ?Sized + 'a, R: RawMutex> MutexRef<'a, T, R> {
	/// Creates a reference to the underlying data of a mutex without
	/// attempting to lock it or take ownership of the key.

	// This might be useful to export, because it makes it easier to express
	// the concept of: "Get the data out the mutex but don't lock it or take
	// the key". But it's also quite dangerous to drop.
	pub(crate) unsafe fn new(mutex: &'a Mutex<T, R>) -> Self {
		Self(mutex, PhantomData)
	}
}

// it's kinda annoying to re-implement some of this stuff on guards
// there's nothing i can do about that

impl<'a, 'key, T: Debug + ?Sized + 'a, Key: Keyable + 'key, R: RawMutex> Debug
	for MutexGuard<'a, 'key, T, Key, R>
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<'a, 'key, T: Display + ?Sized + 'a, Key: Keyable + 'key, R: RawMutex> Display
	for MutexGuard<'a, 'key, T, Key, R>
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawMutex> Deref
	for MutexGuard<'a, 'key, T, Key, R>
{
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.mutex
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawMutex> DerefMut
	for MutexGuard<'a, 'key, T, Key, R>
{
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.mutex
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawMutex> AsRef<T>
	for MutexGuard<'a, 'key, T, Key, R>
{
	fn as_ref(&self) -> &T {
		self
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawMutex> AsMut<T>
	for MutexGuard<'a, 'key, T, Key, R>
{
	fn as_mut(&mut self) -> &mut T {
		self
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawMutex> MutexGuard<'a, 'key, T, Key, R> {
	/// Create a guard to the given mutex. Undefined if multiple guards to the
	/// same mutex exist at once.
	#[must_use]
	pub(super) unsafe fn new(mutex: &'a Mutex<T, R>, thread_key: Key) -> Self {
		Self {
			mutex: MutexRef(mutex, PhantomData),
			thread_key,
			_phantom: PhantomData,
		}
	}
}

unsafe impl<'a, T: ?Sized + Sync + 'a, R: RawMutex + Sync + 'a> Sync for MutexRef<'a, T, R> {}
