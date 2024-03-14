use std::marker::{PhantomData, PhantomPinned};
use std::ptr::NonNull;

use crate::{
	key::Keyable,
	lockable::{Lock, Lockable},
};

mod collection;
mod guard;

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

pub struct BoxedLockCollection<L: 'static>(RefLockCollection<'static, L>);

pub struct PinnedLockCollection<L> {
	_unpin: PhantomPinned,
	data: L,
	locks: Vec<NonNull<dyn Lock>>,
}

unsafe impl<L: Send> Send for PinnedLockCollection<L> {}
unsafe impl<L: Sync> Sync for PinnedLockCollection<L> {}

/// A RAII guard for a generic [`Lockable`] type.
pub struct LockGuard<'a, 'key: 'a, L: Lockable<'a>, Key: Keyable + 'key> {
	guard: L::Guard,
	key: Key,
	_phantom: PhantomData<&'key ()>,
}
