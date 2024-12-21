use std::mem::MaybeUninit;

use crate::rwlock::{ReadLock, RwLock, RwLockReadRef, RwLockWriteRef, WriteLock};

use lock_api::RawRwLock;

/// A raw lock type that may be locked and unlocked
///
/// # Safety
///
/// A deadlock must never occur. The `unlock` method must correctly unlock the
/// data. The `get_ptrs` method must be implemented correctly. The `Output`
/// must be unlocked when it is dropped.
//
// Why not use a RawRwLock? Because that would be semantically incorrect, and I
// don't want an INIT or GuardMarker associated item.
// Originally, RawLock had a sister trait: RawSharableLock. I removed it
// because it'd be difficult to implement a separate type that takes a
// different kind of RawLock. But now the Sharable marker trait is needed to
// indicate if reads can be used.
pub unsafe trait RawLock {
	/// Causes all subsequent calls to the `lock` function on this lock to
	/// panic. This does not affect anything currently holding the lock.
	fn kill(&self);

	/// Blocks until the lock is acquired
	///
	/// # Safety
	///
	/// It is undefined behavior to use this without ownership or mutable
	/// access to the [`ThreadKey`], which should last as long as the return
	/// value is alive.
	///
	/// [`ThreadKey`]: `crate::ThreadKey`
	unsafe fn raw_lock(&self);

	/// Attempt to lock without blocking.
	///
	/// Returns `true` if successful, `false` otherwise.
	///
	/// # Safety
	///
	/// It is undefined behavior to use this without ownership or mutable
	/// access to the [`ThreadKey`], which should last as long as the return
	/// value is alive.
	///
	/// [`ThreadKey`]: `crate::ThreadKey`
	unsafe fn raw_try_lock(&self) -> bool;

	/// Releases the lock
	///
	/// # Safety
	///
	/// It is undefined behavior to use this if the lock is not acquired
	unsafe fn raw_unlock(&self);

	/// Blocks until the data the lock protects can be safely read.
	///
	/// Some locks, but not all, will allow multiple readers at once. If
	/// multiple readers are allowed for a [`Lockable`] type, then the
	/// [`Sharable`] marker trait should be implemented.
	///
	/// # Safety
	///
	/// It is undefined behavior to use this without ownership or mutable
	/// access to the [`ThreadKey`], which should last as long as the return
	/// value is alive.
	///
	/// [`ThreadKey`]: `crate::ThreadKey`
	unsafe fn raw_read(&self);

	// Attempt to read without blocking.
	///
	/// Returns `true` if successful, `false` otherwise.
	///
	/// Some locks, but not all, will allow multiple readers at once. If
	/// multiple readers are allowed for a [`Lockable`] type, then the
	/// [`Sharable`] marker trait should be implemented.
	///
	/// # Safety
	///
	/// It is undefined behavior to use this without ownership or mutable
	/// access to the [`ThreadKey`], which should last as long as the return
	/// value is alive.
	///
	/// [`ThreadKey`]: `crate::ThreadKey`
	unsafe fn raw_try_read(&self) -> bool;

	/// Releases the lock after calling `read`.
	///
	/// # Safety
	///
	/// It is undefined behavior to use this if the read lock is not acquired
	unsafe fn raw_unlock_read(&self);
}

/// A type that may be locked and unlocked.
///
/// This trait is usually implemented on collections of [`RawLock`]s. For
/// example, a `Vec<Mutex<i32>>`.
///
/// # Safety
///
/// Acquiring the locks returned by `get_ptrs` must allow for the values
/// returned by `guard` or `read_guard` to be safely used for exclusive or
/// shared access, respectively.
///
/// Dropping the `Guard` and `ReadGuard` types must unlock those same locks.
///
/// The order of the resulting list from `get_ptrs` must be deterministic. As
/// long as the value is not mutated, the references must always be in the same
/// order.
pub unsafe trait Lockable {
	/// The exclusive guard that does not hold a key
	type Guard<'g>
	where
		Self: 'g;

	/// The shared guard type that does not hold a key
	type ReadGuard<'g>
	where
		Self: 'g;

	/// Yields a list of references to the [`RawLock`]s contained within this
	/// value.
	///
	/// These reference locks which must be locked before acquiring a guard,
	/// and unlocked when the guard is dropped. The order of the resulting list
	/// is deterministic. As long as the value is not mutated, the references
	/// will always be in the same order.
	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>);

	/// Returns a guard that can be used to access the underlying data mutably.
	///
	/// # Safety
	///
	/// All locks given by calling [`Lockable::get_ptrs`] must be locked
	/// exclusively before calling this function. The locks must not be
	/// unlocked until this guard is dropped.
	#[must_use]
	unsafe fn guard(&self) -> Self::Guard<'_>;

	/// Returns a guard that can be used to immutably access the underlying
	/// data.
	///
	/// # Safety
	///
	/// All locks given by calling [`Lockable::get_ptrs`] must be locked using
	/// [`RawLock::read`] before calling this function. The locks must not be
	/// unlocked until this guard is dropped.
	#[must_use]
	unsafe fn read_guard(&self) -> Self::ReadGuard<'_>;
}

/// A trait which indicates that `into_inner` is a valid operation for a
/// [`Lockable`].
///
/// This is used for types like [`Poisonable`] to access the inner value of a
/// lock. [`Poisonable::into_inner`] calls [`LockableIntoInner::into_inner`] to
/// return a mutable reference of the inner value. This isn't implemented for
/// some `Lockable`s, such as `&[T]`.
///
/// [`Poisonable`]: `crate::Poisonable`
/// [`Poisonable::into_inner`]: `crate::poisonable::Poisonable::into_inner`
pub trait LockableIntoInner: Lockable {
	/// The inner type that is behind the lock
	type Inner;

	/// Consumes the lock, returning the underlying the lock.
	fn into_inner(self) -> Self::Inner;
}

/// A trait which indicates that `as_mut` is a valid operation for a
/// [`Lockable`].
///
/// This is used for types like [`Poisonable`] to access the inner value of a
/// lock. [`Poisonable::get_mut`] calls [`LockableAsMut::as_mut`] to return a
/// mutable reference of the inner value. This isn't implemented for some
/// `Lockable`s, such as `&[T]`.
///
/// [`Poisonable`]: `crate::Poisonable`
/// [`Poisonable::get_mut`]: `crate::poisonable::Poisonable::get_mut`
pub trait LockableAsMut: Lockable {
	/// The inner type that is behind the lock
	type Inner<'a>
	where
		Self: 'a;

	/// Returns a mutable reference to the underlying data.
	fn as_mut(&mut self) -> Self::Inner<'_>;
}

/// A marker trait to indicate that multiple readers can access the lock at a
/// time.
///
/// # Safety
///
/// This type must only be implemented if the lock can be safely shared between
/// multiple readers.
pub unsafe trait Sharable: Lockable {}

/// A type that may be locked and unlocked, and is known to be the only valid
/// instance of the lock.
///
/// # Safety
///
/// There must not be any two values which can unlock the value at the same
/// time, i.e., this must either be an owned value or a mutable reference.
pub unsafe trait OwnedLockable: Lockable {}

unsafe impl<T: Send, R: RawRwLock + Send + Sync> Lockable for ReadLock<'_, T, R> {
	type Guard<'g>
		= RwLockReadRef<'g, T, R>
	where
		Self: 'g;

	type ReadGuard<'g>
		= RwLockReadRef<'g, T, R>
	where
		Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		ptrs.push(self.as_ref());
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		RwLockReadRef::new(self.as_ref())
	}

	unsafe fn read_guard(&self) -> Self::Guard<'_> {
		RwLockReadRef::new(self.as_ref())
	}
}

unsafe impl<T: Send, R: RawRwLock + Send + Sync> Lockable for WriteLock<'_, T, R> {
	type Guard<'g>
		= RwLockWriteRef<'g, T, R>
	where
		Self: 'g;

	type ReadGuard<'g>
		= RwLockWriteRef<'g, T, R>
	where
		Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		ptrs.push(self.as_ref());
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		RwLockWriteRef::new(self.as_ref())
	}

	unsafe fn read_guard(&self) -> Self::Guard<'_> {
		RwLockWriteRef::new(self.as_ref())
	}
}

// Technically, the exclusive locks can also be shared, but there's currently
// no way to express that. I don't think I want to ever express that.
unsafe impl<T: Send, R: RawRwLock + Send + Sync> Sharable for ReadLock<'_, T, R> {}

// Because both ReadLock and WriteLock hold references to RwLocks, they can't
// implement OwnedLockable

unsafe impl<T: Lockable> Lockable for &T {
	type Guard<'g>
		= T::Guard<'g>
	where
		Self: 'g;

	type ReadGuard<'g>
		= T::ReadGuard<'g>
	where
		Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		(*self).get_ptrs(ptrs);
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		(*self).guard()
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		(*self).read_guard()
	}
}

unsafe impl<T: Sharable> Sharable for &T {}

unsafe impl<T: Lockable> Lockable for &mut T {
	type Guard<'g>
		= T::Guard<'g>
	where
		Self: 'g;

	type ReadGuard<'g>
		= T::ReadGuard<'g>
	where
		Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		(**self).get_ptrs(ptrs)
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		(**self).guard()
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		(**self).read_guard()
	}
}

impl<T: LockableAsMut> LockableAsMut for &mut T {
	type Inner<'a>
		= T::Inner<'a>
	where
		Self: 'a;

	fn as_mut(&mut self) -> Self::Inner<'_> {
		(*self).as_mut()
	}
}

unsafe impl<T: Sharable> Sharable for &mut T {}

unsafe impl<T: OwnedLockable> OwnedLockable for &mut T {}

/// Implements `Lockable`, `Sharable`, and `OwnedLockable` for tuples
/// ex: `tuple_impls!(A B C, 0 1 2);`
macro_rules! tuple_impls {
	($($generic:ident)*, $($value:tt)*) => {
		unsafe impl<$($generic: Lockable,)*> Lockable for ($($generic,)*) {
			type Guard<'g> = ($($generic::Guard<'g>,)*) where Self: 'g;

			type ReadGuard<'g> = ($($generic::ReadGuard<'g>,)*) where Self: 'g;

			fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
				self.0.get_ptrs(ptrs);
			}

			unsafe fn guard(&self) -> Self::Guard<'_> {
				// It's weird that this works
				// I don't think any other way of doing it compiles
				($(self.$value.guard(),)*)
			}

			unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
				($(self.$value.read_guard(),)*)
			}
		}

		impl<$($generic: LockableAsMut,)*> LockableAsMut for ($($generic,)*) {
			type Inner<'a> = ($($generic::Inner<'a>,)*) where Self: 'a;

			fn as_mut(&mut self) -> Self::Inner<'_> {
				($(self.$value.as_mut(),)*)
			}
		}

		impl<$($generic: LockableIntoInner,)*> LockableIntoInner for ($($generic,)*) {
			type Inner = ($($generic::Inner,)*);

			fn into_inner(self) -> Self::Inner {
				($(self.$value.into_inner(),)*)
			}
		}

		unsafe impl<$($generic: Sharable,)*> Sharable for ($($generic,)*) {}

		unsafe impl<$($generic: OwnedLockable,)*> OwnedLockable for ($($generic,)*) {}
	};
}

tuple_impls!(A, 0);
tuple_impls!(A B, 0 1);
tuple_impls!(A B C, 0 1 2);
tuple_impls!(A B C D, 0 1 2 3);
tuple_impls!(A B C D E, 0 1 2 3 4);
tuple_impls!(A B C D E F, 0 1 2 3 4 5);
tuple_impls!(A B C D E F G, 0 1 2 3 4 5 6);

unsafe impl<T: Lockable, const N: usize> Lockable for [T; N] {
	type Guard<'g>
		= [T::Guard<'g>; N]
	where
		Self: 'g;

	type ReadGuard<'g>
		= [T::ReadGuard<'g>; N]
	where
		Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		for lock in self {
			lock.get_ptrs(ptrs);
		}
	}

	unsafe fn guard<'g>(&'g self) -> Self::Guard<'g> {
		// The MaybeInit helper functions for arrays aren't stable yet, so
		// we'll just have to implement it ourselves
		let mut guards = MaybeUninit::<[MaybeUninit<T::Guard<'g>>; N]>::uninit().assume_init();
		for i in 0..N {
			guards[i].write(self[i].guard());
		}

		guards.map(|g| g.assume_init())
	}

	unsafe fn read_guard<'g>(&'g self) -> Self::ReadGuard<'g> {
		let mut guards = MaybeUninit::<[MaybeUninit<T::ReadGuard<'g>>; N]>::uninit().assume_init();
		for i in 0..N {
			guards[i].write(self[i].read_guard());
		}

		guards.map(|g| g.assume_init())
	}
}

unsafe impl<T: Lockable> Lockable for Box<[T]> {
	type Guard<'g>
		= Box<[T::Guard<'g>]>
	where
		Self: 'g;

	type ReadGuard<'g>
		= Box<[T::ReadGuard<'g>]>
	where
		Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		for lock in self.iter() {
			lock.get_ptrs(ptrs);
		}
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		self.iter().map(|lock| lock.guard()).collect()
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		self.iter().map(|lock| lock.read_guard()).collect()
	}
}

unsafe impl<T: Lockable> Lockable for Vec<T> {
	// There's no reason why I'd ever want to extend a list of lock guards
	type Guard<'g>
		= Box<[T::Guard<'g>]>
	where
		Self: 'g;

	type ReadGuard<'g>
		= Box<[T::ReadGuard<'g>]>
	where
		Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		for lock in self {
			lock.get_ptrs(ptrs);
		}
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		self.iter().map(|lock| lock.guard()).collect()
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		self.iter().map(|lock| lock.read_guard()).collect()
	}
}

// I'd make a generic impl<T: Lockable, I: IntoIterator<Item=T>> Lockable for I
// but I think that'd require sealing up this trait

impl<T: LockableAsMut, const N: usize> LockableAsMut for [T; N] {
	type Inner<'a>
		= [T::Inner<'a>; N]
	where
		Self: 'a;

	fn as_mut(&mut self) -> Self::Inner<'_> {
		unsafe {
			let mut guards = MaybeUninit::<[MaybeUninit<T::Inner<'_>>; N]>::uninit().assume_init();
			for (i, lock) in self.iter_mut().enumerate() {
				guards[i].write(lock.as_mut());
			}

			guards.map(|g| g.assume_init())
		}
	}
}

impl<T: LockableIntoInner, const N: usize> LockableIntoInner for [T; N] {
	type Inner = [T::Inner; N];

	fn into_inner(self) -> Self::Inner {
		unsafe {
			let mut guards = MaybeUninit::<[MaybeUninit<T::Inner>; N]>::uninit().assume_init();
			for (i, lock) in self.into_iter().enumerate() {
				guards[i].write(lock.into_inner());
			}

			guards.map(|g| g.assume_init())
		}
	}
}

impl<T: LockableAsMut + 'static> LockableAsMut for Box<[T]> {
	type Inner<'a>
		= Box<[T::Inner<'a>]>
	where
		Self: 'a;

	fn as_mut(&mut self) -> Self::Inner<'_> {
		self.iter_mut().map(LockableAsMut::as_mut).collect()
	}
}

// TODO: using edition 2024, impl LockableIntoInner for Box<[T]>

impl<T: LockableAsMut + 'static> LockableAsMut for Vec<T> {
	type Inner<'a>
		= Box<[T::Inner<'a>]>
	where
		Self: 'a;

	fn as_mut(&mut self) -> Self::Inner<'_> {
		self.iter_mut().map(LockableAsMut::as_mut).collect()
	}
}

impl<T: LockableIntoInner> LockableIntoInner for Vec<T> {
	type Inner = Box<[T::Inner]>;

	fn into_inner(self) -> Self::Inner {
		self.into_iter()
			.map(LockableIntoInner::into_inner)
			.collect()
	}
}

unsafe impl<T: Sharable, const N: usize> Sharable for [T; N] {}
unsafe impl<T: Sharable> Sharable for Box<[T]> {}
unsafe impl<T: Sharable> Sharable for Vec<T> {}

unsafe impl<T: OwnedLockable, const N: usize> OwnedLockable for [T; N] {}
unsafe impl<T: OwnedLockable> OwnedLockable for Box<[T]> {}
unsafe impl<T: OwnedLockable> OwnedLockable for Vec<T> {}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{Mutex, RwLock};

	#[test]
	fn mut_ref_get_ptrs() {
		let mut rwlock = RwLock::new(5);
		let mutref = &mut rwlock;
		let mut lock_ptrs = Vec::new();
		mutref.get_ptrs(&mut lock_ptrs);

		assert_eq!(lock_ptrs.len(), 1);
		assert!(std::ptr::addr_eq(lock_ptrs[0], mutref));
	}

	#[test]
	fn read_lock_get_ptrs() {
		let rwlock = RwLock::new(5);
		let readlock = ReadLock::new(&rwlock);
		let mut lock_ptrs = Vec::new();
		readlock.get_ptrs(&mut lock_ptrs);

		assert_eq!(lock_ptrs.len(), 1);
		assert!(std::ptr::addr_eq(lock_ptrs[0], &rwlock));
	}

	#[test]
	fn write_lock_get_ptrs() {
		let rwlock = RwLock::new(5);
		let writelock = WriteLock::new(&rwlock);
		let mut lock_ptrs = Vec::new();
		writelock.get_ptrs(&mut lock_ptrs);

		assert_eq!(lock_ptrs.len(), 1);
		assert!(std::ptr::addr_eq(lock_ptrs[0], &rwlock));
	}

	#[test]
	fn array_get_ptrs_empty() {
		let locks: [Mutex<()>; 0] = [];
		let mut lock_ptrs = Vec::new();
		locks.get_ptrs(&mut lock_ptrs);

		assert!(lock_ptrs.is_empty());
	}

	#[test]
	fn array_get_ptrs_length_one() {
		let locks: [Mutex<i32>; 1] = [Mutex::new(1)];
		let mut lock_ptrs = Vec::new();
		locks.get_ptrs(&mut lock_ptrs);

		assert_eq!(lock_ptrs.len(), 1);
		unsafe { assert!(std::ptr::addr_eq(lock_ptrs[0], locks[0].raw())) }
	}

	#[test]
	fn array_get_ptrs_length_two() {
		let locks: [Mutex<i32>; 2] = [Mutex::new(1), Mutex::new(2)];
		let mut lock_ptrs = Vec::new();
		locks.get_ptrs(&mut lock_ptrs);

		assert_eq!(lock_ptrs.len(), 2);
		unsafe { assert!(std::ptr::addr_eq(lock_ptrs[0], locks[0].raw())) }
		unsafe { assert!(std::ptr::addr_eq(lock_ptrs[1], locks[1].raw())) }
	}

	#[test]
	fn vec_get_ptrs_empty() {
		let locks: Vec<Mutex<()>> = Vec::new();
		let mut lock_ptrs = Vec::new();
		locks.get_ptrs(&mut lock_ptrs);

		assert!(lock_ptrs.is_empty());
	}

	#[test]
	fn vec_get_ptrs_length_one() {
		let locks: Vec<Mutex<i32>> = vec![Mutex::new(1)];
		let mut lock_ptrs = Vec::new();
		locks.get_ptrs(&mut lock_ptrs);

		assert_eq!(lock_ptrs.len(), 1);
		unsafe { assert!(std::ptr::addr_eq(lock_ptrs[0], locks[0].raw())) }
	}

	#[test]
	fn vec_get_ptrs_length_two() {
		let locks: Vec<Mutex<i32>> = vec![Mutex::new(1), Mutex::new(2)];
		let mut lock_ptrs = Vec::new();
		locks.get_ptrs(&mut lock_ptrs);

		assert_eq!(lock_ptrs.len(), 2);
		unsafe { assert!(std::ptr::addr_eq(lock_ptrs[0], locks[0].raw())) }
		unsafe { assert!(std::ptr::addr_eq(lock_ptrs[1], locks[1].raw())) }
	}

	#[test]
	fn vec_as_mut() {
		let mut locks: Vec<Mutex<i32>> = vec![Mutex::new(1), Mutex::new(2)];
		let lock_ptrs = LockableAsMut::as_mut(&mut locks);

		assert_eq!(lock_ptrs.len(), 2);
		assert_eq!(*lock_ptrs[0], 1);
		assert_eq!(*lock_ptrs[1], 2);
	}

	#[test]
	fn vec_into_inner() {
		let locks: Vec<Mutex<i32>> = vec![Mutex::new(1), Mutex::new(2)];
		let lock_ptrs = LockableIntoInner::into_inner(locks);

		assert_eq!(lock_ptrs.len(), 2);
		assert_eq!(lock_ptrs[0], 1);
		assert_eq!(lock_ptrs[1], 2);
	}

	#[test]
	fn box_get_ptrs_empty() {
		let locks: Box<[Mutex<()>]> = Box::from([]);
		let mut lock_ptrs = Vec::new();
		locks.get_ptrs(&mut lock_ptrs);

		assert!(lock_ptrs.is_empty());
	}

	#[test]
	fn box_get_ptrs_length_one() {
		let locks: Box<[Mutex<i32>]> = vec![Mutex::new(1)].into_boxed_slice();
		let mut lock_ptrs = Vec::new();
		locks.get_ptrs(&mut lock_ptrs);

		assert_eq!(lock_ptrs.len(), 1);
		unsafe { assert!(std::ptr::addr_eq(lock_ptrs[0], locks[0].raw())) }
	}

	#[test]
	fn box_get_ptrs_length_two() {
		let locks: Box<[Mutex<i32>]> = vec![Mutex::new(1), Mutex::new(2)].into_boxed_slice();
		let mut lock_ptrs = Vec::new();
		locks.get_ptrs(&mut lock_ptrs);

		assert_eq!(lock_ptrs.len(), 2);
		unsafe { assert!(std::ptr::addr_eq(lock_ptrs[0], locks[0].raw())) }
		unsafe { assert!(std::ptr::addr_eq(lock_ptrs[1], locks[1].raw())) }
	}

	#[test]
	fn box_as_mut() {
		let mut locks: Box<[Mutex<i32>]> = vec![Mutex::new(1), Mutex::new(2)].into_boxed_slice();
		let lock_ptrs = LockableAsMut::as_mut(&mut locks);

		assert_eq!(lock_ptrs.len(), 2);
		assert_eq!(*lock_ptrs[0], 1);
		assert_eq!(*lock_ptrs[1], 2);
	}
}
