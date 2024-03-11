use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use lock_api::RawMutex;

use crate::key::Keyable;

use super::{Mutex, MutexGuard, MutexRef};

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

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawMutex> MutexGuard<'a, 'key, T, Key, R> {
	/// Create a guard to the given mutex. Undefined if multiple guards to the
	/// same mutex exist at once.
	#[must_use]
	pub(super) const unsafe fn new(mutex: &'a Mutex<T, R>, thread_key: Key) -> Self {
		Self {
			mutex: MutexRef(mutex),
			thread_key,
			_phantom: PhantomData,
		}
	}
}
