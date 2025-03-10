use std::mem::MaybeUninit;

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
	fn poison(&self);

	/// Blocks until the lock is acquired
	///
	/// # Safety
	///
	/// It is undefined behavior to use this without ownership or mutable
	/// access to the [`ThreadKey`], which should last as long as the return
	/// value is alive.
	///
	/// [`ThreadKey`]: `crate::ThreadKey`
	unsafe fn raw_write(&self);

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
	unsafe fn raw_try_write(&self) -> bool;

	/// Releases the lock
	///
	/// # Safety
	///
	/// It is undefined behavior to use this if the lock is not acquired
	unsafe fn raw_unlock_write(&self);

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
/// Acquiring the locks returned by `get_ptrs` must allow access to the values
/// returned by `guard`.
///
/// Dropping the `Guard` must unlock those same locks.
///
/// The order of the resulting list from `get_ptrs` must be deterministic. As
/// long as the value is not mutated, the references must always be in the same
/// order.
pub unsafe trait Lockable {
	/// The exclusive guard that does not hold a key
	type Guard<'g>
	where
		Self: 'g;

	type DataMut<'a>
	where
		Self: 'a;

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

	#[must_use]
	unsafe fn data_mut(&self) -> Self::DataMut<'_>;
}

/// Allows a lock to be accessed by multiple readers.
///
/// # Safety
///
/// Acquiring shared access to the locks returned by `get_ptrs` must allow
/// shared access to the values returned by `read_guard`.
///
/// Dropping the `ReadGuard` must unlock those same locks.
pub unsafe trait Sharable: Lockable {
	/// The shared guard type that does not hold a key
	type ReadGuard<'g>
	where
		Self: 'g;

	type DataRef<'a>
	where
		Self: 'a;

	/// Returns a guard that can be used to immutably access the underlying
	/// data.
	///
	/// # Safety
	///
	/// All locks given by calling [`Lockable::get_ptrs`] must be locked using
	/// [`RawLock::raw_read`] before calling this function. The locks must not be
	/// unlocked until this guard is dropped.
	#[must_use]
	unsafe fn read_guard(&self) -> Self::ReadGuard<'_>;

	#[must_use]
	unsafe fn data_ref(&self) -> Self::DataRef<'_>;
}

/// A type that may be locked and unlocked, and is known to be the only valid
/// instance of the lock.
///
/// # Safety
///
/// There must not be any two values which can unlock the value at the same
/// time, i.e., this must either be an owned value or a mutable reference.
pub unsafe trait OwnedLockable: Lockable {}

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
/// lock. [`Poisonable::get_mut`] calls [`LockableGetMut::get_mut`] to return a
/// mutable reference of the inner value. This isn't implemented for some
/// `Lockable`s, such as `&[T]`.
///
/// [`Poisonable`]: `crate::Poisonable`
/// [`Poisonable::get_mut`]: `crate::poisonable::Poisonable::get_mut`
pub trait LockableGetMut: Lockable {
	/// The inner type that is behind the lock
	type Inner<'a>
	where
		Self: 'a;

	/// Returns a mutable reference to the underlying data.
	fn get_mut(&mut self) -> Self::Inner<'_>;
}

unsafe impl<T: Lockable> Lockable for &T {
	type Guard<'g>
		= T::Guard<'g>
	where
		Self: 'g;

	type DataMut<'a>
		= T::DataMut<'a>
	where
		Self: 'a;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		(*self).get_ptrs(ptrs);
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		(*self).guard()
	}

	unsafe fn data_mut(&self) -> Self::DataMut<'_> {
		(*self).data_mut()
	}
}

unsafe impl<T: Sharable> Sharable for &T {
	type ReadGuard<'g>
		= T::ReadGuard<'g>
	where
		Self: 'g;

	type DataRef<'a>
		= T::DataRef<'a>
	where
		Self: 'a;

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		(*self).read_guard()
	}

	unsafe fn data_ref(&self) -> Self::DataRef<'_> {
		(*self).data_ref()
	}
}

unsafe impl<T: Lockable> Lockable for &mut T {
	type Guard<'g>
		= T::Guard<'g>
	where
		Self: 'g;

	type DataMut<'a>
		= T::DataMut<'a>
	where
		Self: 'a;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		(**self).get_ptrs(ptrs)
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		(**self).guard()
	}

	unsafe fn data_mut(&self) -> Self::DataMut<'_> {
		(**self).data_mut()
	}
}

impl<T: LockableGetMut> LockableGetMut for &mut T {
	type Inner<'a>
		= T::Inner<'a>
	where
		Self: 'a;

	fn get_mut(&mut self) -> Self::Inner<'_> {
		(*self).get_mut()
	}
}

unsafe impl<T: Sharable> Sharable for &mut T {
	type ReadGuard<'g>
		= T::ReadGuard<'g>
	where
		Self: 'g;

	type DataRef<'a>
		= T::DataRef<'a>
	where
		Self: 'a;

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		(**self).read_guard()
	}

	unsafe fn data_ref(&self) -> Self::DataRef<'_> {
		(**self).data_ref()
	}
}

unsafe impl<T: OwnedLockable> OwnedLockable for &mut T {}

/// Implements `Lockable`, `Sharable`, and `OwnedLockable` for tuples
/// ex: `tuple_impls!(A B C, 0 1 2);`
macro_rules! tuple_impls {
	($($generic:ident)*, $($value:tt)*) => {
		unsafe impl<$($generic: Lockable,)*> Lockable for ($($generic,)*) {
			type Guard<'g> = ($($generic::Guard<'g>,)*) where Self: 'g;

			type DataMut<'a> = ($($generic::DataMut<'a>,)*) where Self: 'a;

			fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
				$(self.$value.get_ptrs(ptrs));*
			}

			unsafe fn guard(&self) -> Self::Guard<'_> {
				// It's weird that this works
				// I don't think any other way of doing it compiles
				($(self.$value.guard(),)*)
			}

			unsafe fn data_mut(&self) -> Self::DataMut<'_> {
				($(self.$value.data_mut(),)*)
			}
		}

		impl<$($generic: LockableGetMut,)*> LockableGetMut for ($($generic,)*) {
			type Inner<'a> = ($($generic::Inner<'a>,)*) where Self: 'a;

			fn get_mut(&mut self) -> Self::Inner<'_> {
				($(self.$value.get_mut(),)*)
			}
		}

		impl<$($generic: LockableIntoInner,)*> LockableIntoInner for ($($generic,)*) {
			type Inner = ($($generic::Inner,)*);

			fn into_inner(self) -> Self::Inner {
				($(self.$value.into_inner(),)*)
			}
		}

		unsafe impl<$($generic: Sharable,)*> Sharable for ($($generic,)*) {
			type ReadGuard<'g> = ($($generic::ReadGuard<'g>,)*) where Self: 'g;

			type DataRef<'a> = ($($generic::DataRef<'a>,)*) where Self: 'a;

			unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
				($(self.$value.read_guard(),)*)
			}

			unsafe fn data_ref(&self) -> Self::DataRef<'_> {
				($(self.$value.data_ref(),)*)
			}
		}

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

	type DataMut<'a>
		= [T::DataMut<'a>; N]
	where
		Self: 'a;

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

	unsafe fn data_mut<'a>(&'a self) -> Self::DataMut<'a> {
		let mut guards = MaybeUninit::<[MaybeUninit<T::DataMut<'a>>; N]>::uninit().assume_init();
		for i in 0..N {
			guards[i].write(self[i].data_mut());
		}

		guards.map(|g| g.assume_init())
	}
}

impl<T: LockableGetMut, const N: usize> LockableGetMut for [T; N] {
	type Inner<'a>
		= [T::Inner<'a>; N]
	where
		Self: 'a;

	fn get_mut(&mut self) -> Self::Inner<'_> {
		unsafe {
			let mut guards = MaybeUninit::<[MaybeUninit<T::Inner<'_>>; N]>::uninit().assume_init();
			for (i, lock) in self.iter_mut().enumerate() {
				guards[i].write(lock.get_mut());
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

unsafe impl<T: Sharable, const N: usize> Sharable for [T; N] {
	type ReadGuard<'g>
		= [T::ReadGuard<'g>; N]
	where
		Self: 'g;

	type DataRef<'a>
		= [T::DataRef<'a>; N]
	where
		Self: 'a;

	unsafe fn read_guard<'g>(&'g self) -> Self::ReadGuard<'g> {
		let mut guards = MaybeUninit::<[MaybeUninit<T::ReadGuard<'g>>; N]>::uninit().assume_init();
		for i in 0..N {
			guards[i].write(self[i].read_guard());
		}

		guards.map(|g| g.assume_init())
	}

	unsafe fn data_ref<'a>(&'a self) -> Self::DataRef<'a> {
		let mut guards = MaybeUninit::<[MaybeUninit<T::DataRef<'a>>; N]>::uninit().assume_init();
		for i in 0..N {
			guards[i].write(self[i].data_ref());
		}

		guards.map(|g| g.assume_init())
	}
}

unsafe impl<T: OwnedLockable, const N: usize> OwnedLockable for [T; N] {}

unsafe impl<T: Lockable> Lockable for Box<[T]> {
	type Guard<'g>
		= Box<[T::Guard<'g>]>
	where
		Self: 'g;

	type DataMut<'a>
		= Box<[T::DataMut<'a>]>
	where
		Self: 'a;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		for lock in self {
			lock.get_ptrs(ptrs);
		}
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		self.iter().map(|lock| lock.guard()).collect()
	}

	unsafe fn data_mut(&self) -> Self::DataMut<'_> {
		self.iter().map(|lock| lock.data_mut()).collect()
	}
}

impl<T: LockableGetMut + 'static> LockableGetMut for Box<[T]> {
	type Inner<'a>
		= Box<[T::Inner<'a>]>
	where
		Self: 'a;

	fn get_mut(&mut self) -> Self::Inner<'_> {
		self.iter_mut().map(LockableGetMut::get_mut).collect()
	}
}

unsafe impl<T: Sharable> Sharable for Box<[T]> {
	type ReadGuard<'g>
		= Box<[T::ReadGuard<'g>]>
	where
		Self: 'g;

	type DataRef<'a>
		= Box<[T::DataRef<'a>]>
	where
		Self: 'a;

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		self.iter().map(|lock| lock.read_guard()).collect()
	}

	unsafe fn data_ref(&self) -> Self::DataRef<'_> {
		self.iter().map(|lock| lock.data_ref()).collect()
	}
}

unsafe impl<T: Sharable> Sharable for Vec<T> {
	type ReadGuard<'g>
		= Box<[T::ReadGuard<'g>]>
	where
		Self: 'g;

	type DataRef<'a>
		= Box<[T::DataRef<'a>]>
	where
		Self: 'a;

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		self.iter().map(|lock| lock.read_guard()).collect()
	}

	unsafe fn data_ref(&self) -> Self::DataRef<'_> {
		self.iter().map(|lock| lock.data_ref()).collect()
	}
}

unsafe impl<T: OwnedLockable> OwnedLockable for Box<[T]> {}

unsafe impl<T: Lockable> Lockable for Vec<T> {
	// There's no reason why I'd ever want to extend a list of lock guards
	type Guard<'g>
		= Box<[T::Guard<'g>]>
	where
		Self: 'g;

	type DataMut<'a>
		= Box<[T::DataMut<'a>]>
	where
		Self: 'a;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		for lock in self {
			lock.get_ptrs(ptrs);
		}
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		self.iter().map(|lock| lock.guard()).collect()
	}

	unsafe fn data_mut(&self) -> Self::DataMut<'_> {
		self.iter().map(|lock| lock.data_mut()).collect()
	}
}

// I'd make a generic impl<T: Lockable, I: IntoIterator<Item=T>> Lockable for I
// but I think that'd require sealing up this trait

// TODO: using edition 2024, impl LockableIntoInner for Box<[T]>

impl<T: LockableGetMut + 'static> LockableGetMut for Vec<T> {
	type Inner<'a>
		= Box<[T::Inner<'a>]>
	where
		Self: 'a;

	fn get_mut(&mut self) -> Self::Inner<'_> {
		self.iter_mut().map(LockableGetMut::get_mut).collect()
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

unsafe impl<T: OwnedLockable> OwnedLockable for Vec<T> {}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{LockCollection, Mutex, RwLock, ThreadKey};

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
		let lock_ptrs = LockableGetMut::get_mut(&mut locks);

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
	fn vec_guard_ref() {
		let key = ThreadKey::get().unwrap();
		let locks = vec![RwLock::new(1), RwLock::new(2)];
		let collection = LockCollection::new(locks);

		let mut guard = collection.lock(key);
		assert_eq!(*guard[0], 1);
		assert_eq!(*guard[1], 2);
		*guard[0] = 3;

		let key = LockCollection::<Vec<RwLock<_>>>::unlock(guard);
		let guard = collection.read(key);
		assert_eq!(*guard[0], 3);
		assert_eq!(*guard[1], 2);
	}

	#[test]
	fn vec_data_mut() {
		let mut key = ThreadKey::get().unwrap();
		let mutexes = vec![Mutex::new(1), Mutex::new(2)];
		let collection = LockCollection::new(mutexes);
		collection.scoped_lock(&mut key, |guard| {
			assert_eq!(*guard[0], 1);
			assert_eq!(*guard[1], 2);
			*guard[0] = 3;
		});

		collection.scoped_lock(&mut key, |guard| {
			assert_eq!(*guard[0], 3);
			assert_eq!(*guard[1], 2);
		})
	}

	#[test]
	fn vec_data_ref() {
		let mut key = ThreadKey::get().unwrap();
		let mutexes = vec![RwLock::new(1), RwLock::new(2)];
		let collection = LockCollection::new(mutexes);
		collection.scoped_lock(&mut key, |guard| {
			assert_eq!(*guard[0], 1);
			assert_eq!(*guard[1], 2);
			*guard[0] = 3;
		});

		collection.scoped_read(&mut key, |guard| {
			assert_eq!(*guard[0], 3);
			assert_eq!(*guard[1], 2);
		})
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
		let lock_ptrs = LockableGetMut::get_mut(&mut locks);

		assert_eq!(lock_ptrs.len(), 2);
		assert_eq!(*lock_ptrs[0], 1);
		assert_eq!(*lock_ptrs[1], 2);
	}

	#[test]
	fn box_guard_mut() {
		let key = ThreadKey::get().unwrap();
		let x = [Mutex::new(1), Mutex::new(2)];
		let collection: LockCollection<Box<[Mutex<_>]>> = LockCollection::new(Box::new(x));

		let mut guard = collection.lock(key);
		assert_eq!(*guard[0], 1);
		assert_eq!(*guard[1], 2);
		*guard[0] = 3;

		let key = LockCollection::<Box<[Mutex<_>]>>::unlock(guard);
		let guard = collection.lock(key);
		assert_eq!(*guard[0], 3);
		assert_eq!(*guard[1], 2);
	}

	#[test]
	fn box_data_mut() {
		let mut key = ThreadKey::get().unwrap();
		let mutexes = vec![Mutex::new(1), Mutex::new(2)].into_boxed_slice();
		let collection = LockCollection::new(mutexes);
		collection.scoped_lock(&mut key, |guard| {
			assert_eq!(*guard[0], 1);
			assert_eq!(*guard[1], 2);
			*guard[0] = 3;
		});

		collection.scoped_lock(&mut key, |guard| {
			assert_eq!(*guard[0], 3);
			assert_eq!(*guard[1], 2);
		});
	}

	#[test]
	fn box_guard_ref() {
		let key = ThreadKey::get().unwrap();
		let locks = [RwLock::new(1), RwLock::new(2)];
		let collection: LockCollection<Box<[RwLock<_>]>> = LockCollection::new(Box::new(locks));

		let mut guard = collection.lock(key);
		assert_eq!(*guard[0], 1);
		assert_eq!(*guard[1], 2);
		*guard[0] = 3;

		let key = LockCollection::<Box<[RwLock<_>]>>::unlock(guard);
		let guard = collection.read(key);
		assert_eq!(*guard[0], 3);
		assert_eq!(*guard[1], 2);
	}

	#[test]
	fn box_data_ref() {
		let mut key = ThreadKey::get().unwrap();
		let mutexes = vec![RwLock::new(1), RwLock::new(2)].into_boxed_slice();
		let collection = LockCollection::new(mutexes);
		collection.scoped_lock(&mut key, |guard| {
			assert_eq!(*guard[0], 1);
			assert_eq!(*guard[1], 2);
			*guard[0] = 3;
		});

		collection.scoped_read(&mut key, |guard| {
			assert_eq!(*guard[0], 3);
			assert_eq!(*guard[1], 2);
		});
	}
}
