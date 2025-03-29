use std::cell::UnsafeCell;

use crate::{lockable::RawLock, ThreadKey};

mod boxed;
mod guard;
mod owned;
mod r#ref;
mod retry;
pub(crate) mod utils;

/// Locks a collection of locks, which cannot be shared immutably.
///
/// This could be a tuple of [`Lockable`] types, an array, or a `Vec`. But it
/// can be safely locked without causing a deadlock.
///
/// The data in this collection is guaranteed to not contain duplicates because
/// `L` must always implement [`OwnedLockable`]. The underlying data may not be
/// immutably referenced. Because of this, there is no need for sorting the
/// locks in the collection, or checking for duplicates, because it can be
/// guaranteed that until the underlying collection is mutated (which requires
/// releasing all acquired locks in the collection to do), then the locks will
/// stay in the same order and be locked in that order, preventing cyclic wait.
///
/// [`Lockable`]: `crate::lockable::Lockable`
/// [`OwnedLockable`]: `crate::lockable::OwnedLockable`

// this type caches the idea that no immutable references to the underlying
// collection exist
#[derive(Debug)]
pub struct OwnedLockCollection<L> {
	// TODO: rename to child
	data: L,
}

/// Locks a reference to a collection of locks, by sorting them by memory
/// address.
///
/// This could be a tuple of [`Lockable`] types, an array, or a `Vec`. But it
/// can be safely locked without causing a deadlock.
///
/// Upon construction, it must be confirmed that the collection contains no
/// duplicate locks. This can be done by either using [`OwnedLockable`] or by
/// checking. Regardless of how this is done, the locks will be sorted by their
/// memory address before locking them. The sorted order of the locks is cached
/// within this collection.
///
/// Unlike [`BoxedLockCollection`], this type does not allocate memory for the
/// data, although it does allocate memory for the sorted list of lock
/// references. This makes it slightly faster, but lifetimes must be handled.
///
/// [`Lockable`]: `crate::lockable::Lockable`
/// [`OwnedLockable`]: `crate::lockable::OwnedLockable`
//
// This type was born when I eventually realized that I needed a self
// referential structure. That used boxing, so I elected to make a more
// efficient implementation (polonius please save us)
//
// This type caches the sorting order of the locks and the fact that it doesn't
// contain any duplicates.
pub struct RefLockCollection<'a, L> {
	data: &'a L,
	locks: Vec<&'a dyn RawLock>,
}

/// Locks a collection of locks, stored in the heap, by sorting them by memory
/// address.
///
/// This could be a tuple of [`Lockable`] types, an array, or a `Vec`. But it
/// can be safely locked without causing a deadlock.
///
/// Upon construction, it must be confirmed that the collection contains no
/// duplicate locks. This can be done by either using [`OwnedLockable`] or by
/// checking. Regardless of how this is done, the locks will be sorted by their
/// memory address before locking them. The sorted order of the locks is cached
/// within this collection.
///
/// Unlike [`RefLockCollection`], this is a self-referential type which boxes
/// the data that is given to it. This means no lifetimes are necessary on the
/// type itself, but it is slightly slower because of the memory allocation.
///
/// [`Lockable`]: `crate::lockable::Lockable`
/// [`OwnedLockable`]: `crate::lockable::OwnedLockable`
//
// This type caches the sorting order of the locks and the fact that it doesn't
// contain any duplicates.
pub struct BoxedLockCollection<L> {
	data: *const UnsafeCell<L>,
	locks: Vec<&'static dyn RawLock>,
}

/// Locks a collection of locks using a retrying algorithm.
///
/// This could be a tuple of [`Lockable`] types, an array, or a `Vec`. But it
/// can be safely locked without causing a deadlock.
///
/// The data in this collection is guaranteed to not contain duplicates, but it
/// also is not sorted. In some cases the lack of sorting can increase
/// performance. However, in most cases, this collection will be slower. Cyclic
/// wait is not guaranteed here, so the locking algorithm must release all its
/// locks if one of the lock attempts blocks. This results in wasted time and
/// potential [livelocking].
///
/// However, one case where this might be faster than [`RefLockCollection`] is
/// when cyclic wait is ensured manually. This will prevent the need for
/// subsequent unlocking and re-locking.
///
/// [`Lockable`]: `crate::lockable::Lockable`
/// [`OwnedLockable`]: `crate::lockable::OwnedLockable`
/// [livelocking]: https://en.wikipedia.org/wiki/Deadlock#Livelock
//
// This type caches the fact that there are no duplicates
#[derive(Debug)]
pub struct RetryingLockCollection<L> {
	data: L,
}

/// A RAII guard for a generic [`Lockable`] type. When this structure is
/// dropped (falls out of scope), the locks will be unlocked.
///
/// The data protected by the mutex can be accessed through this guard via its
/// [`Deref`] and [`DerefMut`] implementations.
///
/// Several lock collections can be used to create this type. Specifically,
/// [`BoxedLockCollection`], [`RefLockCollection`], [`OwnedLockCollection`], and
/// [`RetryingLockCollection`]. It is created using the methods, `lock`,
/// `try_lock`, `read`, and `try_read`.
///
/// [`Deref`]: `std::ops::Deref`
/// [`DerefMut`]: `std::ops::DerefMut`
/// [`Lockable`]: `crate::lockable::Lockable`
pub struct LockGuard<Guard> {
	guard: Guard,
	key: ThreadKey,
}
