use std::fmt::Debug;

use lock_api::RawRwLock;

use crate::key::Keyable;

use super::{ReadLock, RwLock, RwLockReadGuard, RwLockReadRef};

impl<'a, T: ?Sized, R> Debug for ReadLock<'a, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&format!("ReadLock<{}>", std::any::type_name::<T>()))
	}
}

impl<'a, T: ?Sized, R> From<&'a RwLock<T, R>> for ReadLock<'a, T, R> {
	fn from(value: &'a RwLock<T, R>) -> Self {
		Self::new(value)
	}
}

impl<'a, T: ?Sized, R> AsRef<RwLock<T, R>> for ReadLock<'a, T, R> {
	fn as_ref(&self) -> &RwLock<T, R> {
		self.0
	}
}

impl<'a, T: ?Sized, R> ReadLock<'a, T, R> {
	#[must_use]
	pub const fn new(rwlock: &'a RwLock<T, R>) -> Self {
		Self(rwlock)
	}
}

impl<'a, T: ?Sized, R: RawRwLock> ReadLock<'a, T, R> {
	pub fn lock<'s, 'key: 's, Key: Keyable + 'key>(
		&'s self,
		key: Key,
	) -> RwLockReadGuard<'_, 'key, T, Key, R> {
		self.0.read(key)
	}

	pub(crate) unsafe fn lock_no_key(&self) -> RwLockReadRef<'_, T, R> {
		self.0.read_no_key()
	}

	pub fn try_lock<'s, 'key: 's, Key: Keyable + 'key>(
		&'s self,
		key: Key,
	) -> Option<RwLockReadGuard<'_, 'key, T, Key, R>> {
		self.0.try_read(key)
	}

	pub(crate) unsafe fn try_lock_no_key(&self) -> Option<RwLockReadRef<'_, T, R>> {
		self.0.try_read_no_key()
	}

	pub fn unlock<'key, Key: Keyable + 'key>(guard: RwLockReadGuard<'_, 'key, T, Key, R>) -> Key {
		RwLock::unlock_read(guard)
	}
}
