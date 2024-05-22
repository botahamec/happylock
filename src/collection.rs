use std::marker::PhantomData;

use crate::{key::Keyable, lockable::Lock, Lockable, Sharable};

mod boxed;
mod guard;
mod owned;
mod readonly;
mod r#ref;
mod retry;

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
pub struct LockGuard<'key, Guard, Key: Keyable + 'key> {
	guard: Guard,
	key: Key,
	_phantom: PhantomData<&'key ()>,
}

pub struct Readonly<Collection> {
	collection: Collection,
}

mod sealed {
	#[allow(clippy::wildcard_imports)]
	use super::*;

	pub trait Sealed {}

	impl<L> Sealed for OwnedLockCollection<L> {}
	impl<'a, L> Sealed for RefLockCollection<'a, L> {}
	impl<'a, L> Sealed for BoxedLockCollection<'a, L> {}
	impl<L> Sealed for RetryingLockCollection<L> {}
}

pub trait LockCollection<L: Lockable>: sealed::Sealed {
	unsafe fn new_readonly(data: L) -> Self
	where
		L: Sharable;

	fn read<'g, 'key, Key: Keyable + 'key>(
		&self,
		key: Key,
	) -> LockGuard<'key, L::ReadGuard<'g>, Key>
	where
		L: Sharable;

	fn try_read<'g, 'key, Key: Keyable + 'key>(
		&self,
		key: Key,
	) -> Option<LockGuard<'key, L::ReadGuard<'g>, Key>>
	where
		L: Sharable;

	fn unlock_read<'key, Key: Keyable + 'key>(guard: LockGuard<'key, L::ReadGuard<'_>, Key>) -> Key
	where
		L: Sharable;
}
