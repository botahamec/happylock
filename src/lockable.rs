use std::mem::MaybeUninit;

use crate::{
	mutex::{Mutex, MutexRef},
	rwlock::{ReadLock, RwLock, RwLockReadRef, RwLockWriteRef, WriteLock},
};

use lock_api::{RawMutex, RawRwLock};

/// A raw lock type that may be locked and unlocked
///
/// # Safety
///
/// A deadlock must never occur. The `unlock` method must correctly unlock the
/// data. The `get_ptrs` method must be implemented correctly. The `Output`
/// must be unlocked when it is dropped.
pub unsafe trait RawLock: Send + Sync {
	/// Blocks until the lock is acquired
	///
	/// # Safety
	///
	/// It is undefined behavior to use this without ownership or mutable
	/// access to the [`ThreadKey`], which should last as long as the return
	/// value is alive.
	///
	/// [`ThreadKey`]: `crate::ThreadKey`
	unsafe fn lock(&self);

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
	unsafe fn try_lock(&self) -> bool;

	/// Releases the lock
	///
	/// # Safety
	///
	/// It is undefined behavior to use this if the lock is not acquired
	unsafe fn unlock(&self);

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
	unsafe fn read(&self);

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
	unsafe fn try_read(&self) -> bool;

	/// Releases the lock after calling `read`.
	///
	/// # Safety
	///
	/// It is undefined behavior to use this if the read lock is not acquired
	unsafe fn unlock_read(&self);
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

unsafe impl<T: Send, R: RawMutex + Send + Sync> RawLock for Mutex<T, R> {
	unsafe fn lock(&self) {
		self.raw().lock()
	}

	unsafe fn try_lock(&self) -> bool {
		self.raw().try_lock()
	}

	unsafe fn unlock(&self) {
		self.raw().unlock()
	}

	unsafe fn read(&self) {
		self.raw().lock()
	}

	unsafe fn try_read(&self) -> bool {
		self.raw().try_lock()
	}

	unsafe fn unlock_read(&self) {
		self.raw().unlock()
	}
}

unsafe impl<T: Send, R: RawRwLock + Send + Sync> RawLock for RwLock<T, R> {
	unsafe fn lock(&self) {
		self.raw().lock_exclusive()
	}

	unsafe fn try_lock(&self) -> bool {
		self.raw().try_lock_exclusive()
	}

	unsafe fn unlock(&self) {
		self.raw().unlock_exclusive()
	}

	unsafe fn read(&self) {
		self.raw().lock_shared()
	}

	unsafe fn try_read(&self) -> bool {
		self.raw().try_lock_shared()
	}

	unsafe fn unlock_read(&self) {
		self.raw().unlock_shared()
	}
}

unsafe impl<T: Send, R: RawMutex + Send + Sync> Lockable for Mutex<T, R> {
	type Guard<'g> = MutexRef<'g, T, R> where Self: 'g;
	type ReadGuard<'g> = MutexRef<'g, T, R> where Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		ptrs.push(self);
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		MutexRef::new(self)
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		MutexRef::new(self)
	}
}

unsafe impl<T: Send, R: RawRwLock + Send + Sync> Lockable for RwLock<T, R> {
	type Guard<'g> = RwLockWriteRef<'g, T, R> where Self: 'g;

	type ReadGuard<'g> = RwLockReadRef<'g, T, R> where Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		ptrs.push(self);
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		RwLockWriteRef::new(self)
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		RwLockReadRef::new(self)
	}
}

unsafe impl<T: Send, R: RawRwLock + Send + Sync> Sharable for RwLock<T, R> {}

unsafe impl<T: Send, R: RawMutex + Send + Sync> OwnedLockable for Mutex<T, R> {}

unsafe impl<T: Send, R: RawRwLock + Send + Sync> OwnedLockable for RwLock<T, R> {}

unsafe impl<'l, T: Send, R: RawRwLock + Send + Sync> Lockable for ReadLock<'l, T, R> {
	type Guard<'g> = RwLockReadRef<'g, T, R> where Self: 'g;

	type ReadGuard<'g> = RwLockReadRef<'g, T, R> where Self: 'g;

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

unsafe impl<'l, T: Send, R: RawRwLock + Send + Sync> Lockable for WriteLock<'l, T, R> {
	type Guard<'g> = RwLockWriteRef<'g, T, R> where Self: 'g;

	type ReadGuard<'g> = RwLockWriteRef<'g, T, R> where Self: 'g;

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

unsafe impl<'l, T: Send, R: RawRwLock + Send + Sync> Sharable for ReadLock<'l, T, R> {}

unsafe impl<T: Lockable> Lockable for &T {
	type Guard<'g> = T::Guard<'g> where Self: 'g;

	type ReadGuard<'g> = T::ReadGuard<'g> where Self: 'g;

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

unsafe impl<T: Lockable> Lockable for &mut T {
	type Guard<'g> = T::Guard<'g> where Self: 'g;

	type ReadGuard<'g> = T::ReadGuard<'g> where Self: 'g;

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

unsafe impl<T: OwnedLockable> OwnedLockable for &mut T {}

unsafe impl<A: Lockable> Lockable for (A,) {
	type Guard<'g> = (A::Guard<'g>,) where Self: 'g;

	type ReadGuard<'g> = (A::ReadGuard<'g>,) where Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		self.0.get_ptrs(ptrs);
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		(self.0.guard(),)
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		(self.0.read_guard(),)
	}
}

unsafe impl<A: Lockable, B: Lockable> Lockable for (A, B) {
	type Guard<'g> = (A::Guard<'g>, B::Guard<'g>) where Self: 'g;

	type ReadGuard<'g> = (A::ReadGuard<'g>, B::ReadGuard<'g>) where Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		self.0.get_ptrs(ptrs);
		self.1.get_ptrs(ptrs);
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		(self.0.guard(), self.1.guard())
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		(self.0.read_guard(), self.1.read_guard())
	}
}

unsafe impl<A: Lockable, B: Lockable, C: Lockable> Lockable for (A, B, C) {
	type Guard<'g> = (A::Guard<'g>, B::Guard<'g>, C::Guard<'g>) where Self: 'g;

	type ReadGuard<'g> = (A::ReadGuard<'g>, B::ReadGuard<'g>, C::ReadGuard<'g>) where Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		self.0.get_ptrs(ptrs);
		self.1.get_ptrs(ptrs);
		self.2.get_ptrs(ptrs);
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		(self.0.guard(), self.1.guard(), self.2.guard())
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		(
			self.0.read_guard(),
			self.1.read_guard(),
			self.2.read_guard(),
		)
	}
}

unsafe impl<A: Lockable, B: Lockable, C: Lockable, D: Lockable> Lockable for (A, B, C, D) {
	type Guard<'g> = (A::Guard<'g>, B::Guard<'g>, C::Guard<'g>, D::Guard<'g>) where Self: 'g;

	type ReadGuard<'g> = (
		A::ReadGuard<'g>,
		B::ReadGuard<'g>,
		C::ReadGuard<'g>,
		D::ReadGuard<'g>,
	) where Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		self.0.get_ptrs(ptrs);
		self.1.get_ptrs(ptrs);
		self.2.get_ptrs(ptrs);
		self.3.get_ptrs(ptrs);
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		(
			self.0.guard(),
			self.1.guard(),
			self.2.guard(),
			self.3.guard(),
		)
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		(
			self.0.read_guard(),
			self.1.read_guard(),
			self.2.read_guard(),
			self.3.read_guard(),
		)
	}
}

unsafe impl<A: Lockable, B: Lockable, C: Lockable, D: Lockable, E: Lockable> Lockable
	for (A, B, C, D, E)
{
	type Guard<'g> = (
		A::Guard<'g>,
		B::Guard<'g>,
		C::Guard<'g>,
		D::Guard<'g>,
		E::Guard<'g>,
	) where Self: 'g;

	type ReadGuard<'g> = (
		A::ReadGuard<'g>,
		B::ReadGuard<'g>,
		C::ReadGuard<'g>,
		D::ReadGuard<'g>,
		E::ReadGuard<'g>,
	) where Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		self.0.get_ptrs(ptrs);
		self.1.get_ptrs(ptrs);
		self.2.get_ptrs(ptrs);
		self.3.get_ptrs(ptrs);
		self.4.get_ptrs(ptrs);
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		(
			self.0.guard(),
			self.1.guard(),
			self.2.guard(),
			self.3.guard(),
			self.4.guard(),
		)
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		(
			self.0.read_guard(),
			self.1.read_guard(),
			self.2.read_guard(),
			self.3.read_guard(),
			self.4.read_guard(),
		)
	}
}

unsafe impl<A: Lockable, B: Lockable, C: Lockable, D: Lockable, E: Lockable, F: Lockable> Lockable
	for (A, B, C, D, E, F)
{
	type Guard<'g> = (
		A::Guard<'g>,
		B::Guard<'g>,
		C::Guard<'g>,
		D::Guard<'g>,
		E::Guard<'g>,
		F::Guard<'g>,
	) where Self: 'g;

	type ReadGuard<'g> = (
		A::ReadGuard<'g>,
		B::ReadGuard<'g>,
		C::ReadGuard<'g>,
		D::ReadGuard<'g>,
		E::ReadGuard<'g>,
		F::ReadGuard<'g>,
	) where Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		self.0.get_ptrs(ptrs);
		self.1.get_ptrs(ptrs);
		self.2.get_ptrs(ptrs);
		self.3.get_ptrs(ptrs);
		self.4.get_ptrs(ptrs);
		self.5.get_ptrs(ptrs);
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		(
			self.0.guard(),
			self.1.guard(),
			self.2.guard(),
			self.3.guard(),
			self.4.guard(),
			self.5.guard(),
		)
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		(
			self.0.read_guard(),
			self.1.read_guard(),
			self.2.read_guard(),
			self.3.read_guard(),
			self.4.read_guard(),
			self.5.read_guard(),
		)
	}
}

unsafe impl<A: Lockable, B: Lockable, C: Lockable, D: Lockable, E: Lockable, F: Lockable, G: Lockable>
	Lockable for (A, B, C, D, E, F, G)
{
	type Guard<'g> = (
		A::Guard<'g>,
		B::Guard<'g>,
		C::Guard<'g>,
		D::Guard<'g>,
		E::Guard<'g>,
		F::Guard<'g>,
		G::Guard<'g>,
	) where Self: 'g;

	type ReadGuard<'g> = (
		A::ReadGuard<'g>,
		B::ReadGuard<'g>,
		C::ReadGuard<'g>,
		D::ReadGuard<'g>,
		E::ReadGuard<'g>,
		F::ReadGuard<'g>,
		G::ReadGuard<'g>,
	) where Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		self.0.get_ptrs(ptrs);
		self.1.get_ptrs(ptrs);
		self.2.get_ptrs(ptrs);
		self.3.get_ptrs(ptrs);
		self.4.get_ptrs(ptrs);
		self.5.get_ptrs(ptrs);
		self.6.get_ptrs(ptrs);
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		(
			self.0.guard(),
			self.1.guard(),
			self.2.guard(),
			self.3.guard(),
			self.4.guard(),
			self.5.guard(),
			self.6.guard(),
		)
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		(
			self.0.read_guard(),
			self.1.read_guard(),
			self.2.read_guard(),
			self.3.read_guard(),
			self.4.read_guard(),
			self.5.read_guard(),
			self.6.read_guard(),
		)
	}
}

unsafe impl<A: Sharable> Sharable for (A,) {}
unsafe impl<A: Sharable, B: Sharable> Sharable for (A, B) {}

unsafe impl<A: Sharable, B: Sharable, C: Sharable> Sharable for (A, B, C) {}

unsafe impl<A: Sharable, B: Sharable, C: Sharable, D: Sharable> Sharable for (A, B, C, D) {}

unsafe impl<A: Sharable, B: Sharable, C: Sharable, D: Sharable, E: Sharable> Sharable
	for (A, B, C, D, E)
{
}

unsafe impl<A: Sharable, B: Sharable, C: Sharable, D: Sharable, E: Sharable, F: Sharable> Sharable
	for (A, B, C, D, E, F)
{
}

unsafe impl<A: Sharable, B: Sharable, C: Sharable, D: Sharable, E: Sharable, F: Sharable, G: Sharable>
	Sharable for (A, B, C, D, E, F, G)
{
}

unsafe impl<A: OwnedLockable> OwnedLockable for (A,) {}
unsafe impl<A: OwnedLockable, B: OwnedLockable> OwnedLockable for (A, B) {}

unsafe impl<A: OwnedLockable, B: OwnedLockable, C: OwnedLockable> OwnedLockable for (A, B, C) {}

unsafe impl<A: OwnedLockable, B: OwnedLockable, C: OwnedLockable, D: OwnedLockable> OwnedLockable
	for (A, B, C, D)
{
}

unsafe impl<A: OwnedLockable, B: OwnedLockable, C: OwnedLockable, D: OwnedLockable, E: OwnedLockable>
	OwnedLockable for (A, B, C, D, E)
{
}

unsafe impl<
		A: OwnedLockable,
		B: OwnedLockable,
		C: OwnedLockable,
		D: OwnedLockable,
		E: OwnedLockable,
		F: OwnedLockable,
	> OwnedLockable for (A, B, C, D, E, F)
{
}

unsafe impl<
		A: OwnedLockable,
		B: OwnedLockable,
		C: OwnedLockable,
		D: OwnedLockable,
		E: OwnedLockable,
		F: OwnedLockable,
		G: OwnedLockable,
	> OwnedLockable for (A, B, C, D, E, F, G)
{
}

unsafe impl<T: Lockable, const N: usize> Lockable for [T; N] {
	type Guard<'g> = [T::Guard<'g>; N] where Self: 'g;

	type ReadGuard<'g> = [T::ReadGuard<'g>; N] where Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		for lock in self {
			lock.get_ptrs(ptrs);
		}
	}

	unsafe fn guard<'g>(&'g self) -> Self::Guard<'g> {
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
	type Guard<'g> = Box<[T::Guard<'g>]> where Self: 'g;

	type ReadGuard<'g> = Box<[T::ReadGuard<'g>]> where Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		for lock in self.iter() {
			lock.get_ptrs(ptrs);
		}
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		let mut guards = Vec::new();
		for lock in self.iter() {
			guards.push(lock.guard());
		}

		guards.into_boxed_slice()
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		let mut guards = Vec::new();
		for lock in self.iter() {
			guards.push(lock.read_guard());
		}

		guards.into_boxed_slice()
	}
}

unsafe impl<T: Lockable> Lockable for Vec<T> {
	type Guard<'g> = Vec<T::Guard<'g>> where Self: 'g;

	type ReadGuard<'g> = Box<[T::ReadGuard<'g>]> where Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		for lock in self {
			lock.get_ptrs(ptrs);
		}
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		let mut guards = Vec::new();
		for lock in self {
			guards.push(lock.guard());
		}

		guards
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		let mut guards = Vec::new();
		for lock in self {
			guards.push(lock.read_guard());
		}

		guards.into_boxed_slice()
	}
}

unsafe impl<T: Sharable, const N: usize> Sharable for [T; N] {}
unsafe impl<T: Sharable> Sharable for Box<[T]> {}
unsafe impl<T: Sharable> Sharable for Vec<T> {}

unsafe impl<T: OwnedLockable, const N: usize> OwnedLockable for [T; N] {}
unsafe impl<T: OwnedLockable> OwnedLockable for Box<[T]> {}
unsafe impl<T: OwnedLockable> OwnedLockable for Vec<T> {}
