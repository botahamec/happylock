use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use lock_api::RawMutex;

use crate::key::Keyable;
use crate::lockable::RawLock;

use super::{Mutex, MutexGuard, MutexRef};

// These impls make things slightly easier because now you can use
// `println!("{guard}")` instead of `println!("{}", *guard)`

impl<T: PartialEq + ?Sized, R: RawMutex> PartialEq for MutexRef<'_, T, R> {
	fn eq(&self, other: &Self) -> bool {
		self.deref().eq(&**other)
	}
}

impl<T: Eq + ?Sized, R: RawMutex> Eq for MutexRef<'_, T, R> {}

impl<T: PartialOrd + ?Sized, R: RawMutex> PartialOrd for MutexRef<'_, T, R> {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		self.deref().partial_cmp(&**other)
	}
}

impl<T: Ord + ?Sized, R: RawMutex> Ord for MutexRef<'_, T, R> {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.deref().cmp(&**other)
	}
}

impl<T: Hash + ?Sized, R: RawMutex> Hash for MutexRef<'_, T, R> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.deref().hash(state)
	}
}

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
		unsafe { self.0.raw_unlock() }
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
	pub(crate) unsafe fn new(mutex: &'a Mutex<T, R>) -> Self {
		Self(mutex, PhantomData)
	}
}

// it's kinda annoying to re-implement some of this stuff on guards
// there's nothing i can do about that

impl<T: PartialEq + ?Sized, R: RawMutex, Key: Keyable> PartialEq for MutexGuard<'_, '_, T, Key, R> {
	fn eq(&self, other: &Self) -> bool {
		self.deref().eq(&**other)
	}
}

impl<T: Eq + ?Sized, R: RawMutex, Key: Keyable> Eq for MutexGuard<'_, '_, T, Key, R> {}

impl<T: PartialOrd + ?Sized, R: RawMutex, Key: Keyable> PartialOrd
	for MutexGuard<'_, '_, T, Key, R>
{
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		self.deref().partial_cmp(&**other)
	}
}

impl<T: Ord + ?Sized, R: RawMutex, Key: Keyable> Ord for MutexGuard<'_, '_, T, Key, R> {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.deref().cmp(&**other)
	}
}

impl<T: Hash + ?Sized, R: RawMutex, Key: Keyable> Hash for MutexGuard<'_, '_, T, Key, R> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.deref().hash(state)
	}
}

impl<T: Debug + ?Sized, Key: Keyable, R: RawMutex> Debug for MutexGuard<'_, '_, T, Key, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<T: Display + ?Sized, Key: Keyable, R: RawMutex> Display for MutexGuard<'_, '_, T, Key, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}

impl<T: ?Sized, Key: Keyable, R: RawMutex> Deref for MutexGuard<'_, '_, T, Key, R> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.mutex
	}
}

impl<T: ?Sized, Key: Keyable, R: RawMutex> DerefMut for MutexGuard<'_, '_, T, Key, R> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.mutex
	}
}

impl<T: ?Sized, Key: Keyable, R: RawMutex> AsRef<T> for MutexGuard<'_, '_, T, Key, R> {
	fn as_ref(&self) -> &T {
		self
	}
}

impl<T: ?Sized, Key: Keyable, R: RawMutex> AsMut<T> for MutexGuard<'_, '_, T, Key, R> {
	fn as_mut(&mut self) -> &mut T {
		self
	}
}

impl<'a, T: ?Sized, Key: Keyable, R: RawMutex> MutexGuard<'a, '_, T, Key, R> {
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

unsafe impl<T: ?Sized + Sync, R: RawMutex + Sync> Sync for MutexRef<'_, T, R> {}
