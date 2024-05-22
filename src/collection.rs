use std::marker::PhantomData;

use crate::{key::Keyable, lockable::Lock};

mod boxed;
mod guard;
mod owned;
mod r#ref;
mod retry;

#[derive(Debug)]
pub struct OwnedLockCollection<L> {
	data: L,
}

/// A type which can be locked.
///
/// This could be a tuple of [`Lockable`] types, an array, or a `Vec`. But it
/// can be safely locked without causing a deadlock.
pub struct RefLockCollection<'a, L> {
	data: &'a L,
	locks: Vec<&'a dyn Lock>,
}

pub struct BoxedLockCollection<L> {
	data: Box<L>,
	locks: Vec<&'static dyn Lock>,
}

#[derive(Debug)]
pub struct RetryingLockCollection<L> {
	data: L,
}

/// A RAII guard for a generic [`Lockable`] type.
pub struct LockGuard<'key, Guard, Key: Keyable + 'key> {
	guard: Guard,
	key: Key,
	_phantom: PhantomData<&'key ()>,
}
