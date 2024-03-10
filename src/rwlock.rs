use std::cell::UnsafeCell;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use lock_api::RawRwLock;

use crate::key::Keyable;

#[cfg(feature = "spin")]
pub type SpinRwLock<T> = RwLock<T, spin::RwLock<()>>;

#[cfg(feature = "parking_lot")]
pub type ParkingRwLock<T> = RwLock<T, parking_lot::RawRwLock>;

pub struct RwLock<T: ?Sized, R> {
	raw: R,
	value: UnsafeCell<T>,
}

pub struct ReadLock<'a, T: ?Sized, R>(&'a RwLock<T, R>);

pub struct WriteLock<'a, T: ?Sized, R>(&'a RwLock<T, R>);

pub struct RwLockReadRef<'a, T: ?Sized, R: RawRwLock>(&'a RwLock<T, R>);

pub struct RwLockWriteRef<'a, T: ?Sized, R: RawRwLock>(&'a RwLock<T, R>);

pub struct RwLockReadGuard<'a, 'key, T: ?Sized, Key: Keyable + 'key, R: RawRwLock> {
	rwlock: RwLockReadRef<'a, T, R>,
	thread_key: Key,
	_phantom: PhantomData<&'key ()>,
}

pub struct RwLockWriteGuard<'a, 'key, T: ?Sized, Key: Keyable + 'key, R: RawRwLock> {
	rwlock: RwLockWriteRef<'a, T, R>,
	thread_key: Key,
	_phantom: PhantomData<&'key ()>,
}

unsafe impl<R: RawRwLock + Send, T: ?Sized + Send> Send for RwLock<T, R> {}
unsafe impl<R: RawRwLock + Sync, T: ?Sized + Send + Sync> Sync for RwLock<T, R> {}

impl<'a, T: ?Sized + 'a, R: RawRwLock> Deref for RwLockReadRef<'a, T, R> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// safety: this is the only type that can use `value`, and there's
		//         a reference to this type, so there cannot be any mutable
		//         references to this value.
		unsafe { &*self.0.value.get() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawRwLock> Drop for RwLockReadRef<'a, T, R> {
	fn drop(&mut self) {
		// safety: this guard is being destroyed, so the data cannot be
		//         accessed without locking again
		unsafe { self.0.force_unlock_read() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawRwLock> Deref for RwLockWriteRef<'a, T, R> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		// safety: this is the only type that can use `value`, and there's
		//         a reference to this type, so there cannot be any mutable
		//         references to this value.
		unsafe { &*self.0.value.get() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawRwLock> DerefMut for RwLockWriteRef<'a, T, R> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		// safety: this is the only type that can use `value`, and we have a
		//         mutable reference to this type, so there cannot be any other
		//         references to this value.
		unsafe { &mut *self.0.value.get() }
	}
}

impl<'a, T: ?Sized + 'a, R: RawRwLock> Drop for RwLockWriteRef<'a, T, R> {
	fn drop(&mut self) {
		// safety: this guard is being destroyed, so the data cannot be
		//         accessed without locking again
		unsafe { self.0.force_unlock_write() }
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawRwLock> Deref
	for RwLockReadGuard<'a, 'key, T, Key, R>
{
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.rwlock
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawRwLock> Deref
	for RwLockWriteGuard<'a, 'key, T, Key, R>
{
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.rwlock
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawRwLock> DerefMut
	for RwLockWriteGuard<'a, 'key, T, Key, R>
{
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.rwlock
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawRwLock>
	RwLockReadGuard<'a, 'key, T, Key, R>
{
	/// Create a guard to the given mutex. Undefined if multiple guards to the
	/// same mutex exist at once.
	#[must_use]
	const unsafe fn new(rwlock: &'a RwLock<T, R>, thread_key: Key) -> Self {
		Self {
			rwlock: RwLockReadRef(rwlock),
			thread_key,
			_phantom: PhantomData,
		}
	}
}

impl<'a, 'key: 'a, T: ?Sized + 'a, Key: Keyable, R: RawRwLock>
	RwLockWriteGuard<'a, 'key, T, Key, R>
{
	/// Create a guard to the given mutex. Undefined if multiple guards to the
	/// same mutex exist at once.
	#[must_use]
	const unsafe fn new(rwlock: &'a RwLock<T, R>, thread_key: Key) -> Self {
		Self {
			rwlock: RwLockWriteRef(rwlock),
			thread_key,
			_phantom: PhantomData,
		}
	}
}

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

impl<'a, T: ?Sized, R> Debug for WriteLock<'a, T, R> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&format!("WriteLock<{}>", std::any::type_name::<T>()))
	}
}

impl<'a, T: ?Sized, R> From<&'a RwLock<T, R>> for WriteLock<'a, T, R> {
	fn from(value: &'a RwLock<T, R>) -> Self {
		Self::new(value)
	}
}

impl<'a, T: ?Sized, R> AsRef<RwLock<T, R>> for WriteLock<'a, T, R> {
	fn as_ref(&self) -> &RwLock<T, R> {
		self.0
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

	unsafe fn force_unlock_read(&self) {
		self.raw.unlock_shared();
	}

	unsafe fn force_unlock_write(&self) {
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

impl<'a, T: ?Sized, R> WriteLock<'a, T, R> {
	#[must_use]
	pub const fn new(rwlock: &'a RwLock<T, R>) -> Self {
		Self(rwlock)
	}
}

impl<'a, T: ?Sized, R: RawRwLock> WriteLock<'a, T, R> {
	pub fn lock<'s, 'key: 's, Key: Keyable + 'key>(
		&'s self,
		key: Key,
	) -> RwLockWriteGuard<'_, 'key, T, Key, R> {
		self.0.write(key)
	}

	pub(crate) unsafe fn lock_no_key(&self) -> RwLockWriteRef<'_, T, R> {
		self.0.write_no_key()
	}

	pub fn try_lock<'s, 'key: 's, Key: Keyable + 'key>(
		&'s self,
		key: Key,
	) -> Option<RwLockWriteGuard<'_, 'key, T, Key, R>> {
		self.0.try_write(key)
	}

	pub(crate) unsafe fn try_lock_no_key(&self) -> Option<RwLockWriteRef<'_, T, R>> {
		self.0.try_write_no_key()
	}

	pub fn unlock<'key, Key: Keyable + 'key>(guard: RwLockWriteGuard<'_, 'key, T, Key, R>) -> Key {
		RwLock::unlock_write(guard)
	}
}
