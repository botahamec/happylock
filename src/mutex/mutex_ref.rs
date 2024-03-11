use std::ops::{Deref, DerefMut};

use lock_api::RawMutex;

use super::MutexRef;

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
		unsafe { &*self.0.value.get() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawMutex> DerefMut for MutexRef<'a, T, R> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		// safety: this is the only type that can use `value`, and we have a
		//         mutable reference to this type, so there cannot be any other
		//         references to this value.
		unsafe { &mut *self.0.value.get() }
	}
}
