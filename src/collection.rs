use std::marker::PhantomData;

use crate::{key::Keyable, lockable::Lockable};

mod collection;
mod guard;

/// A type which can be locked.
///
/// This could be a tuple of [`Lockable`] types, an array, or a `Vec`. But it
/// can be safely locked without causing a deadlock. To do this, it is very
/// important that no duplicate locks are included within.
#[derive(Debug, Clone, Copy)]
pub struct LockCollection<L> {
	data: L,
}

/// A RAII guard for a generic [`Lockable`] type.
pub struct LockGuard<'a, 'key: 'a, L: Lockable<'a>, Key: Keyable + 'key> {
	guard: L::Output,
	key: Key,
	_phantom: PhantomData<&'key ()>,
}
