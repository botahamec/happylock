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
}

pub unsafe trait Lockable<'a> {
	/// The guard returned that does not hold a key
	type Guard;

	fn get_ptrs(&'a self, ptrs: &mut Vec<&'a dyn Lock>);

	#[must_use]
	unsafe fn guard(&'a self) -> Self::Guard;
}

/// A type that may be locked and unlocked, and is known to be the only valid
/// instance of the lock.
///
/// # Safety
///
/// There must not be any two values which can unlock the value at the same
/// time, i.e., this must either be an owned value or a mutable reference.
pub unsafe trait OwnedLockable<'a>: Lockable<'a> {}

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
}

unsafe impl<'a, T: Send + 'a, R: RawMutex + Send + Sync + 'a> Lockable<'a> for Mutex<T, R> {
	type Guard = MutexRef<'a, T, R>;

	fn get_ptrs(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
		ptrs.push(self);
	}

	unsafe fn guard(&'a self) -> Self::Guard {
		MutexRef::new(self)
	}
}

unsafe impl<'a, T: Send + 'a, R: RawRwLock + Send + Sync + 'a> Lockable<'a> for RwLock<T, R> {
	type Guard = RwLockWriteRef<'a, T, R>;

	fn get_ptrs(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
		ptrs.push(self);
	}

	unsafe fn guard(&'a self) -> Self::Guard {
		RwLockWriteRef::new(self)
	}
}

unsafe impl<'a, T: Send + 'a, R: RawMutex + Send + Sync + 'a> OwnedLockable<'a> for Mutex<T, R> {}

unsafe impl<'a, T: Send + 'a, R: RawRwLock + Send + Sync + 'a> OwnedLockable<'a> for RwLock<T, R> {}

unsafe impl<'a, T: Send + 'a, R: RawRwLock + Send + Sync + 'a> Lockable<'a> for ReadLock<'a, T, R> {
	type Guard = RwLockReadRef<'a, T, R>;

	fn get_ptrs(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
		ptrs.push(self.as_ref());
	}

	unsafe fn guard(&'a self) -> Self::Guard {
		RwLockReadRef::new(self.as_ref())
	}
}

unsafe impl<'a, T: Send + 'a, R: RawRwLock + Send + Sync + 'a> Lockable<'a>
	for WriteLock<'a, T, R>
{
	type Guard = RwLockWriteRef<'a, T, R>;

	fn get_ptrs(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
		ptrs.push(self.as_ref());
	}

	unsafe fn guard(&'a self) -> Self::Guard {
		RwLockWriteRef::new(self.as_ref())
	}
}

unsafe impl<'a, T: Lockable<'a>> Lockable<'a> for &'a T {
	type Guard = T::Guard;

	fn get_ptrs(&self, ptrs: &mut Vec<&'a dyn Lock>) {
		(*self).get_ptrs(ptrs);
	}

	unsafe fn guard(&self) -> Self::Guard {
		(*self).guard()
	}
}

unsafe impl<'a, T: Lockable<'a>> Lockable<'a> for &mut T {
	type Guard = T::Guard;

	fn get_ptrs(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
		(**self).get_ptrs(ptrs)
	}

	unsafe fn guard(&'a self) -> Self::Guard {
		(**self).guard()
	}
}

unsafe impl<'a, T: OwnedLockable<'a>> OwnedLockable<'a> for &mut T {}

unsafe impl<'a, A: Lockable<'a>> Lockable<'a> for (A,) {
	type Guard = (A::Guard,);

	fn get_ptrs(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
		self.0.get_ptrs(ptrs);
	}

	unsafe fn guard(&'a self) -> Self::Guard {
		(self.0.guard(),)
	}
}

unsafe impl<'a, A: Lockable<'a>, B: Lockable<'a>> Lockable<'a> for (A, B) {
	type Guard = (A::Guard, B::Guard);

	fn get_ptrs(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
		self.0.get_ptrs(ptrs);
		self.1.get_ptrs(ptrs);
	}

	unsafe fn guard(&'a self) -> Self::Guard {
		(self.0.guard(), self.1.guard())
	}
}

unsafe impl<'a, A: Lockable<'a>, B: Lockable<'a>, C: Lockable<'a>> Lockable<'a> for (A, B, C) {
	type Guard = (A::Guard, B::Guard, C::Guard);

	fn get_ptrs(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
		self.0.get_ptrs(ptrs);
		self.1.get_ptrs(ptrs);
		self.2.get_ptrs(ptrs);
	}

	unsafe fn guard(&'a self) -> Self::Guard {
		(self.0.guard(), self.1.guard(), self.2.guard())
	}
}

unsafe impl<'a, A: Lockable<'a>, B: Lockable<'a>, C: Lockable<'a>, D: Lockable<'a>> Lockable<'a>
	for (A, B, C, D)
{
	type Guard = (A::Guard, B::Guard, C::Guard, D::Guard);

	fn get_ptrs(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
		self.0.get_ptrs(ptrs);
		self.1.get_ptrs(ptrs);
		self.2.get_ptrs(ptrs);
		self.3.get_ptrs(ptrs);
	}

	unsafe fn guard(&'a self) -> Self::Guard {
		(
			self.0.guard(),
			self.1.guard(),
			self.2.guard(),
			self.3.guard(),
		)
	}
}

unsafe impl<'a, A: Lockable<'a>, B: Lockable<'a>, C: Lockable<'a>, D: Lockable<'a>, E: Lockable<'a>>
	Lockable<'a> for (A, B, C, D, E)
{
	type Guard = (A::Guard, B::Guard, C::Guard, D::Guard, E::Guard);

	fn get_ptrs(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
		self.0.get_ptrs(ptrs);
		self.1.get_ptrs(ptrs);
		self.2.get_ptrs(ptrs);
		self.3.get_ptrs(ptrs);
		self.4.get_ptrs(ptrs);
	}

	unsafe fn guard(&'a self) -> Self::Guard {
		(
			self.0.guard(),
			self.1.guard(),
			self.2.guard(),
			self.3.guard(),
			self.4.guard(),
		)
	}
}

unsafe impl<
		'a,
		A: Lockable<'a>,
		B: Lockable<'a>,
		C: Lockable<'a>,
		D: Lockable<'a>,
		E: Lockable<'a>,
		F: Lockable<'a>,
	> Lockable<'a> for (A, B, C, D, E, F)
{
	type Guard = (A::Guard, B::Guard, C::Guard, D::Guard, E::Guard, F::Guard);

	fn get_ptrs(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
		self.0.get_ptrs(ptrs);
		self.1.get_ptrs(ptrs);
		self.2.get_ptrs(ptrs);
		self.3.get_ptrs(ptrs);
		self.4.get_ptrs(ptrs);
		self.5.get_ptrs(ptrs);
	}

	unsafe fn guard(&'a self) -> Self::Guard {
		(
			self.0.guard(),
			self.1.guard(),
			self.2.guard(),
			self.3.guard(),
			self.4.guard(),
			self.5.guard(),
		)
	}
}

unsafe impl<
		'a,
		A: Lockable<'a>,
		B: Lockable<'a>,
		C: Lockable<'a>,
		D: Lockable<'a>,
		E: Lockable<'a>,
		F: Lockable<'a>,
		G: Lockable<'a>,
	> Lockable<'a> for (A, B, C, D, E, F, G)
{
	type Guard = (
		A::Guard,
		B::Guard,
		C::Guard,
		D::Guard,
		E::Guard,
		F::Guard,
		G::Guard,
	);

	fn get_ptrs(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
		self.0.get_ptrs(ptrs);
		self.1.get_ptrs(ptrs);
		self.2.get_ptrs(ptrs);
		self.3.get_ptrs(ptrs);
		self.4.get_ptrs(ptrs);
		self.5.get_ptrs(ptrs);
		self.6.get_ptrs(ptrs);
	}

	unsafe fn guard(&'a self) -> Self::Guard {
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
}

unsafe impl<'a, A: OwnedLockable<'a>> OwnedLockable<'a> for (A,) {}
unsafe impl<'a, A: OwnedLockable<'a>, B: OwnedLockable<'a>> OwnedLockable<'a> for (A, B) {}

unsafe impl<'a, A: OwnedLockable<'a>, B: OwnedLockable<'a>, C: OwnedLockable<'a>> OwnedLockable<'a>
	for (A, B, C)
{
}

unsafe impl<
		'a,
		A: OwnedLockable<'a>,
		B: OwnedLockable<'a>,
		C: OwnedLockable<'a>,
		D: OwnedLockable<'a>,
	> OwnedLockable<'a> for (A, B, C, D)
{
}

unsafe impl<
		'a,
		A: OwnedLockable<'a>,
		B: OwnedLockable<'a>,
		C: OwnedLockable<'a>,
		D: OwnedLockable<'a>,
		E: OwnedLockable<'a>,
	> OwnedLockable<'a> for (A, B, C, D, E)
{
}

unsafe impl<
		'a,
		A: OwnedLockable<'a>,
		B: OwnedLockable<'a>,
		C: OwnedLockable<'a>,
		D: OwnedLockable<'a>,
		E: OwnedLockable<'a>,
		F: OwnedLockable<'a>,
	> OwnedLockable<'a> for (A, B, C, D, E, F)
{
}

unsafe impl<
		'a,
		A: OwnedLockable<'a>,
		B: OwnedLockable<'a>,
		C: OwnedLockable<'a>,
		D: OwnedLockable<'a>,
		E: OwnedLockable<'a>,
		F: OwnedLockable<'a>,
		G: OwnedLockable<'a>,
	> OwnedLockable<'a> for (A, B, C, D, E, F, G)
{
}

unsafe impl<'a, T: Lockable<'a>, const N: usize> Lockable<'a> for [T; N] {
	type Guard = [T::Guard; N];

	fn get_ptrs(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
		for lock in self {
			lock.get_ptrs(ptrs);
		}
	}

	unsafe fn guard(&'a self) -> Self::Guard {
		let mut guards = MaybeUninit::<[MaybeUninit<T::Guard>; N]>::uninit().assume_init();
		for i in 0..N {
			guards[i].write(self[i].guard());
		}

		guards.map(|g| g.assume_init())
	}
}

unsafe impl<'a, T: Lockable<'a>> Lockable<'a> for Box<[T]> {
	type Guard = Box<[T::Guard]>;

	fn get_ptrs(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
		for lock in self.iter() {
			lock.get_ptrs(ptrs);
		}
	}

	unsafe fn guard(&'a self) -> Self::Guard {
		let mut guards = Vec::new();
		for lock in self.iter() {
			guards.push(lock.guard());
		}

		guards.into_boxed_slice()
	}
}

unsafe impl<'a, T: Lockable<'a>> Lockable<'a> for Vec<T> {
	type Guard = Vec<T::Guard>;

	fn get_ptrs(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
		for lock in self {
			lock.get_ptrs(ptrs);
		}
	}

	unsafe fn guard(&'a self) -> Self::Guard {
		let mut guards = Vec::new();
		for lock in self {
			guards.push(lock.guard());
		}

		guards
	}
}

unsafe impl<'a, T: OwnedLockable<'a>, const N: usize> OwnedLockable<'a> for [T; N] {}
unsafe impl<'a, T: OwnedLockable<'a>> OwnedLockable<'a> for Box<[T]> {}
unsafe impl<'a, T: OwnedLockable<'a>> OwnedLockable<'a> for Vec<T> {}
