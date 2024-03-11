use std::cell::UnsafeCell;
use std::fmt::Debug;

use lock_api::RawRwLock;

use crate::key::Keyable;

use super::{RwLock, RwLockReadGuard, RwLockReadRef, RwLockWriteGuard, RwLockWriteRef};

impl<T, R: RawRwLock> RwLock<T, R> {
	#[must_use]
	pub const fn new(value: T) -> Self {
		Self {
			value: UnsafeCell::new(value),
			raw: R::INIT,
		}
	}
}

impl<T: ?Sized, R> Debug for RwLock<T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&format!("RwLock<{}>", std::any::type_name::<T>()))
	}
}

impl<T, R: RawRwLock> From<T> for RwLock<T, R> {
	fn from(value: T) -> Self {
		Self::new(value)
	}
}

impl<T: ?Sized, R> AsMut<T> for RwLock<T, R> {
	fn as_mut(&mut self) -> &mut T {
		self.get_mut()
	}
}

impl<T, R> RwLock<T, R> {
	pub fn into_inner(self) -> T {
		self.value.into_inner()
	}
}

impl<T: ?Sized, R> RwLock<T, R> {
	pub fn get_mut(&mut self) -> &mut T {
		self.value.get_mut()
	}
}

impl<T: ?Sized, R: RawRwLock> RwLock<T, R> {
	pub fn read<'s, 'key: 's, Key: Keyable>(
		&'s self,
		key: Key,
	) -> RwLockReadGuard<'_, 'key, T, Key, R> {
		unsafe {
			self.raw.lock_shared();

			// safety: the lock is locked first
			RwLockReadGuard::new(self, key)
		}
	}

	pub(crate) unsafe fn read_no_key(&self) -> RwLockReadRef<'_, T, R> {
		self.raw.lock_shared();

		// safety: the lock is locked first
		RwLockReadRef(self)
	}

	pub fn try_read<'s, 'key: 's, Key: Keyable>(
		&'s self,
		key: Key,
	) -> Option<RwLockReadGuard<'_, 'key, T, Key, R>> {
		unsafe {
			if self.raw.try_lock_shared() {
				// safety: the lock is locked first
				Some(RwLockReadGuard::new(self, key))
			} else {
				None
			}
		}
	}

	pub(crate) unsafe fn try_read_no_key(&self) -> Option<RwLockReadRef<'_, T, R>> {
		if self.raw.try_lock_shared() {
			// safety: the lock is locked first
			Some(RwLockReadRef(self))
		} else {
			None
		}
	}

	pub fn write<'s, 'key: 's, Key: Keyable>(
		&'s self,
		key: Key,
	) -> RwLockWriteGuard<'_, 'key, T, Key, R> {
		unsafe {
			self.raw.lock_exclusive();

			// safety: the lock is locked first
			RwLockWriteGuard::new(self, key)
		}
	}

	pub(crate) unsafe fn write_no_key(&self) -> RwLockWriteRef<'_, T, R> {
		self.raw.lock_exclusive();

		// safety: the lock is locked first
		RwLockWriteRef(self)
	}

	pub fn try_write<'s, 'key: 's, Key: Keyable>(
		&'s self,
		key: Key,
	) -> Option<RwLockWriteGuard<'_, 'key, T, Key, R>> {
		unsafe {
			if self.raw.try_lock_exclusive() {
				// safety: the lock is locked first
				Some(RwLockWriteGuard::new(self, key))
			} else {
				None
			}
		}
	}

	pub(crate) unsafe fn try_write_no_key(&self) -> Option<RwLockWriteRef<'_, T, R>> {
		if self.raw.try_lock_exclusive() {
			// safety: the lock is locked first
			Some(RwLockWriteRef(self))
		} else {
			None
		}
	}

	pub(super) unsafe fn force_unlock_read(&self) {
		self.raw.unlock_shared();
	}

	pub(super) unsafe fn force_unlock_write(&self) {
		self.raw.unlock_exclusive();
	}

	pub fn unlock_read<'key, Key: Keyable + 'key>(
		guard: RwLockReadGuard<'_, 'key, T, Key, R>,
	) -> Key {
		unsafe {
			guard.rwlock.0.force_unlock_read();
		}
		guard.thread_key
	}

	pub fn unlock_write<'key, Key: Keyable + 'key>(
		guard: RwLockWriteGuard<'_, 'key, T, Key, R>,
	) -> Key {
		unsafe {
			guard.rwlock.0.force_unlock_write();
		}
		guard.thread_key
	}
}

unsafe impl<R: RawRwLock + Send, T: ?Sized + Send> Send for RwLock<T, R> {}
unsafe impl<R: RawRwLock + Sync, T: ?Sized + Send + Sync> Sync for RwLock<T, R> {}
