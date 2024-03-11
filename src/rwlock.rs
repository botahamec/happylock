use std::cell::UnsafeCell;
use std::marker::PhantomData;

use lock_api::RawRwLock;

use crate::key::Keyable;

mod rwlock;

mod read_lock;
mod write_lock;

mod read_guard;
mod write_guard;

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
