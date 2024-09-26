use std::marker::PhantomData;
use std::panic::{RefUnwindSafe, UnwindSafe};

use crate::lockable::{Lockable, RawLock};
use crate::Keyable;

use super::{
	PoisonError, PoisonFlag, PoisonGuard, PoisonRef, PoisonResult, Poisonable,
	TryLockPoisonableError, TryLockPoisonableResult,
};

unsafe impl<L: Lockable + RawLock> Lockable for Poisonable<L> {
	type Guard<'g> = PoisonResult<PoisonRef<'g, L::Guard<'g>>> where Self: 'g;
	type ReadGuard<'g> = PoisonResult<PoisonRef<'g, L::ReadGuard<'g>>> where Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn RawLock>) {
		self.inner.get_ptrs(ptrs)
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		let ref_guard = PoisonRef {
			guard: self.inner.guard(),
			flag: &self.poisoned,
		};

		if self.is_poisoned() {
			Ok(ref_guard)
		} else {
			Err(PoisonError::new(ref_guard))
		}
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		let ref_guard = PoisonRef {
			guard: self.inner.read_guard(),
			flag: &self.poisoned,
		};

		if self.is_poisoned() {
			Ok(ref_guard)
		} else {
			Err(PoisonError::new(ref_guard))
		}
	}
}

impl<L: Lockable + RawLock> From<L> for Poisonable<L> {
	fn from(value: L) -> Self {
		Self::new(value)
	}
}

impl<L: Lockable + RawLock> Poisonable<L> {
	pub const fn new(value: L) -> Self {
		Self {
			inner: value,
			poisoned: PoisonFlag::new(),
		}
	}

	unsafe fn guard<'flag, 'key, Key: Keyable + 'key>(
		&'flag self,
		key: Key,
	) -> PoisonResult<PoisonGuard<'flag, 'key, L::Guard<'flag>, Key>> {
		let guard = PoisonGuard {
			guard: PoisonRef {
				guard: self.inner.guard(),
				flag: &self.poisoned,
			},
			key,
			_phantom: PhantomData,
		};

		if !self.is_poisoned() {
			return Err(PoisonError::new(guard));
		}

		Ok(guard)
	}

	pub fn lock<'flag, 'key, Key: Keyable + 'key>(
		&'flag self,
		key: Key,
	) -> PoisonResult<PoisonGuard<'flag, 'key, L::Guard<'flag>, Key>> {
		unsafe {
			self.inner.lock();
			self.guard(key)
		}
	}

	pub fn try_lock<'flag, 'key, Key: Keyable + 'key>(
		&'flag self,
		key: Key,
	) -> TryLockPoisonableResult<'flag, 'key, L::Guard<'flag>, Key> {
		unsafe {
			if self.inner.try_lock() {
				Ok(self.guard(key)?)
			} else {
				Err(TryLockPoisonableError::WouldBlock(key))
			}
		}
	}

	pub fn unlock<'flag, 'key, Key: Keyable + 'key>(
		guard: PoisonGuard<'flag, 'key, L::Guard<'flag>, Key>,
	) -> Key {
		drop(guard.guard);
		guard.key
	}

	pub fn is_poisoned(&self) -> bool {
		self.poisoned.is_poisoned()
	}

	pub fn clear_poison(&self) {
		self.poisoned.clear_poison()
	}

	pub fn into_inner(self) -> PoisonResult<L> {
		if self.is_poisoned() {
			Err(PoisonError::new(self.inner))
		} else {
			Ok(self.inner)
		}
	}

	pub fn get_mut(&mut self) -> PoisonResult<&mut L> {
		if self.is_poisoned() {
			Err(PoisonError::new(&mut self.inner))
		} else {
			Ok(&mut self.inner)
		}
	}
}

impl<L: Lockable + RawLock> RefUnwindSafe for Poisonable<L> {}
impl<L: Lockable + RawLock> UnwindSafe for Poisonable<L> {}
