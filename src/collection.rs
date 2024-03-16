use std::{marker::PhantomData};

use crate::{
	key::Keyable,
	lockable::{Lock, Lockable},
};

mod boxed_collection;
mod guard;
mod owned_collection;
mod ref_collection;
mod retry_collection;

pub struct OwnedLockCollection<L> {
	data: L,
}

/// A type which can be locked.
///
/// This could be a tuple of [`Lockable`] types, an array, or a `Vec`. But it
/// can be safely locked without causing a deadlock.
pub struct RefLockCollection<'a, L> {
	locks: Vec<&'a dyn Lock>,
	data: &'a L,
}

pub struct BoxedLockCollection<'a, L>(RefLockCollection<'a, L>);

pub struct RetryingLockCollection<L> {
	data: L,
}

/// A RAII guard for a generic [`Lockable`] type.
pub struct LockGuard<'g, 'key: 'g, L: Lockable + 'g, Key: Keyable + 'key> {
	guard: L::Guard<'g>,
	key: Key,
	_phantom: PhantomData<&'key ()>,
}
