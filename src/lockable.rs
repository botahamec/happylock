use std::mem::MaybeUninit;

use crate::{
	mutex::{Mutex, MutexRef},
	rwlock::{ReadLock, RwLock, RwLockReadRef, RwLockWriteRef, WriteLock},
};

use lock_api::{RawMutex, RawRwLock};

/// A type that may be locked and unlocked
///
/// # Safety
///
/// A deadlock must never occur. The `unlock` method must correctly unlock the
/// data. The `get_ptrs` method must be implemented correctly. The `Output`
/// must be unlocked when it is dropped.
pub unsafe trait Lock: Send + Sync {
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

	unsafe fn read(&self);

	unsafe fn try_read(&self) -> bool;

	unsafe fn unlock_read(&self);
}

pub unsafe trait Lockable {
	/// The guard returned that does not hold a key
	type Guard<'g>
	where
		Self: 'g;

	type ReadGuard<'g>
	where
		Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>);

	#[must_use]
	unsafe fn guard(&self) -> Self::Guard<'_>;

	#[must_use]
	unsafe fn read_guard(&self) -> Self::ReadGuard<'_>;
}

pub unsafe trait Sharable: Lockable {}

/// A type that may be locked and unlocked, and is known to be the only valid
/// instance of the lock.
///
/// # Safety
///
/// There must not be any two values which can unlock the value at the same
/// time, i.e., this must either be an owned value or a mutable reference.
pub unsafe trait OwnedLockable: Lockable {}

unsafe impl<T: Send, R: RawMutex + Send + Sync> Lock for Mutex<T, R> {
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

unsafe impl<T: Send, R: RawRwLock + Send + Sync> Lock for RwLock<T, R> {
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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
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
