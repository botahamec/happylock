use std::mem::MaybeUninit;

use crate::{
	mutex::{Mutex, MutexRef},
	rwlock::{ReadLock, RwLock, RwLockReadRef, RwLockWriteRef, WriteLock},
};

use lock_api::{RawMutex, RawRwLock};

/// A type that may be locked and unlocked, and is known to be the only valid
/// instance of the lock.
///
/// # Safety
///
/// There must not be any two values which can unlock the value at the same
/// time, i.e., this must either be an owned value or a mutable reference.
pub unsafe trait OwnedLockable<'a>: Lockable<'a> {}

/// A type that may be locked and unlocked
///
/// # Safety
///
/// A deadlock must never occur. The `unlock` method must correctly unlock the
/// data. The `get_ptrs` method must be implemented correctly. The `Output`
/// must be unlocked when it is dropped.
pub unsafe trait Lockable<'a> {
	/// The output of the lock
	type Output;

	/// Returns a list of all pointers to locks. This is used to ensure that
	/// the same lock isn't included twice
	#[must_use]
	fn get_ptrs(&self) -> Vec<usize>;

	/// Blocks until the lock is acquired
	///
	/// # Safety
	///
	/// It is undefined behavior to:
	/// * Use this without ownership or mutable access to the [`ThreadKey`],
	/// which should last as long as the return value is alive.
	/// * Call this on multiple locks without unlocking first.
	///
	/// [`ThreadKey`]: `crate::ThreadKey`
	unsafe fn lock(&'a self) -> Self::Output;

	/// Attempt to lock without blocking.
	///
	/// Returns `Some` if successful, `None` otherwise.
	///
	/// # Safety
	///
	/// It is undefined behavior to use this without ownership or mutable
	/// access to the [`ThreadKey`], which should last as long as the return
	/// value is alive.
	///
	/// [`ThreadKey`]: `crate::ThreadKey`
	unsafe fn try_lock(&'a self) -> Option<Self::Output>;
}

unsafe impl<'a, T: Lockable<'a>> Lockable<'a> for &T {
	type Output = T::Output;

	fn get_ptrs(&self) -> Vec<usize> {
		(*self).get_ptrs()
	}

	unsafe fn lock(&'a self) -> Self::Output {
		(*self).lock()
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		(*self).try_lock()
	}
}

unsafe impl<'a, T: Lockable<'a>> Lockable<'a> for &mut T {
	type Output = T::Output;

	fn get_ptrs(&self) -> Vec<usize> {
		(**self).get_ptrs()
	}

	unsafe fn lock(&'a self) -> Self::Output {
		(**self).lock()
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		(**self).try_lock()
	}
}

unsafe impl<'a, T: OwnedLockable<'a>> OwnedLockable<'a> for &mut T {}

unsafe impl<'a, T: 'a, R: RawMutex + 'a> Lockable<'a> for Mutex<T, R> {
	type Output = MutexRef<'a, T, R>;

	fn get_ptrs(&self) -> Vec<usize> {
		vec![self as *const Self as usize]
	}

	unsafe fn lock(&'a self) -> Self::Output {
		self.lock_no_key()
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		self.try_lock_no_key()
	}
}

unsafe impl<'a, T: 'a, R: RawRwLock + 'a> Lockable<'a> for RwLock<T, R> {
	type Output = RwLockWriteRef<'a, T, R>;

	fn get_ptrs(&self) -> Vec<usize> {
		vec![self as *const Self as usize]
	}

	unsafe fn lock(&'a self) -> Self::Output {
		self.write_no_key()
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		self.try_write_no_key()
	}
}

unsafe impl<'a, T: 'a, R: RawMutex + 'a> OwnedLockable<'a> for Mutex<T, R> {}

unsafe impl<'a, T: 'a, R: RawRwLock + 'a> OwnedLockable<'a> for RwLock<T, R> {}

unsafe impl<'a, T: 'a, R: RawRwLock + 'a> Lockable<'a> for ReadLock<'a, T, R> {
	type Output = RwLockReadRef<'a, T, R>;

	fn get_ptrs(&self) -> Vec<usize> {
		vec![self.as_ref() as *const RwLock<T, R> as usize]
	}

	unsafe fn lock(&'a self) -> Self::Output {
		self.lock_no_key()
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		self.try_lock_no_key()
	}
}

unsafe impl<'a, T: 'a, R: RawRwLock + 'a> Lockable<'a> for WriteLock<'a, T, R> {
	type Output = RwLockWriteRef<'a, T, R>;

	fn get_ptrs(&self) -> Vec<usize> {
		vec![self.as_ref() as *const RwLock<T, R> as usize]
	}

	unsafe fn lock(&'a self) -> Self::Output {
		self.lock_no_key()
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		self.try_lock_no_key()
	}
}

unsafe impl<'a, A: Lockable<'a>> Lockable<'a> for (A,) {
	type Output = (A::Output,);

	fn get_ptrs(&self) -> Vec<usize> {
		let mut ptrs = Vec::with_capacity(1);
		ptrs.append(&mut self.0.get_ptrs());
		ptrs
	}

	unsafe fn lock(&'a self) -> Self::Output {
		(self.0.lock(),)
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		self.0.try_lock().map(|a| (a,))
	}
}

unsafe impl<'a, A: Lockable<'a>, B: Lockable<'a>> Lockable<'a> for (A, B) {
	type Output = (A::Output, B::Output);

	fn get_ptrs(&self) -> Vec<usize> {
		let mut ptrs = Vec::with_capacity(2);
		ptrs.append(&mut self.0.get_ptrs());
		ptrs.append(&mut self.1.get_ptrs());
		ptrs
	}

	unsafe fn lock(&'a self) -> Self::Output {
		loop {
			let lock0 = self.0.lock();
			let Some(lock1) = self.1.try_lock() else {
				continue;
			};

			return (lock0, lock1);
		}
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		let Some(lock0) = self.0.try_lock() else {
			return None;
		};
		let Some(lock1) = self.1.try_lock() else {
			return None;
		};

		Some((lock0, lock1))
	}
}

unsafe impl<'a, A: Lockable<'a>, B: Lockable<'a>, C: Lockable<'a>> Lockable<'a> for (A, B, C) {
	type Output = (A::Output, B::Output, C::Output);

	fn get_ptrs(&self) -> Vec<usize> {
		let mut ptrs = Vec::with_capacity(3);
		ptrs.append(&mut self.0.get_ptrs());
		ptrs.append(&mut self.1.get_ptrs());
		ptrs.append(&mut self.2.get_ptrs());
		ptrs
	}

	unsafe fn lock(&'a self) -> Self::Output {
		loop {
			let lock0 = self.0.lock();
			let Some(lock1) = self.1.try_lock() else {
				continue;
			};
			let Some(lock2) = self.2.try_lock() else {
				continue;
			};

			return (lock0, lock1, lock2);
		}
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		let Some(lock0) = self.0.try_lock() else {
			return None;
		};
		let Some(lock1) = self.1.try_lock() else {
			return None;
		};
		let Some(lock2) = self.2.try_lock() else {
			return None;
		};

		Some((lock0, lock1, lock2))
	}
}

unsafe impl<'a, A: Lockable<'a>, B: Lockable<'a>, C: Lockable<'a>, D: Lockable<'a>> Lockable<'a>
	for (A, B, C, D)
{
	type Output = (A::Output, B::Output, C::Output, D::Output);

	fn get_ptrs(&self) -> Vec<usize> {
		let mut ptrs = Vec::with_capacity(4);
		ptrs.append(&mut self.0.get_ptrs());
		ptrs.append(&mut self.1.get_ptrs());
		ptrs.append(&mut self.2.get_ptrs());
		ptrs.append(&mut self.3.get_ptrs());
		ptrs
	}

	unsafe fn lock(&'a self) -> Self::Output {
		loop {
			let lock0 = self.0.lock();
			let Some(lock1) = self.1.try_lock() else {
				continue;
			};
			let Some(lock2) = self.2.try_lock() else {
				continue;
			};
			let Some(lock3) = self.3.try_lock() else {
				continue;
			};

			return (lock0, lock1, lock2, lock3);
		}
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		let Some(lock0) = self.0.try_lock() else {
			return None;
		};
		let Some(lock1) = self.1.try_lock() else {
			return None;
		};
		let Some(lock2) = self.2.try_lock() else {
			return None;
		};
		let Some(lock3) = self.3.try_lock() else {
			return None;
		};

		Some((lock0, lock1, lock2, lock3))
	}
}

unsafe impl<'a, A: Lockable<'a>, B: Lockable<'a>, C: Lockable<'a>, D: Lockable<'a>, E: Lockable<'a>>
	Lockable<'a> for (A, B, C, D, E)
{
	type Output = (A::Output, B::Output, C::Output, D::Output, E::Output);

	fn get_ptrs(&self) -> Vec<usize> {
		let mut ptrs = Vec::with_capacity(5);
		ptrs.append(&mut self.0.get_ptrs());
		ptrs.append(&mut self.1.get_ptrs());
		ptrs.append(&mut self.2.get_ptrs());
		ptrs.append(&mut self.3.get_ptrs());
		ptrs.append(&mut self.4.get_ptrs());
		ptrs
	}

	unsafe fn lock(&'a self) -> Self::Output {
		loop {
			let lock0 = self.0.lock();
			let Some(lock1) = self.1.try_lock() else {
				continue;
			};
			let Some(lock2) = self.2.try_lock() else {
				continue;
			};
			let Some(lock3) = self.3.try_lock() else {
				continue;
			};
			let Some(lock4) = self.4.try_lock() else {
				continue;
			};

			return (lock0, lock1, lock2, lock3, lock4);
		}
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		let Some(lock0) = self.0.try_lock() else {
			return None;
		};
		let Some(lock1) = self.1.try_lock() else {
			return None;
		};
		let Some(lock2) = self.2.try_lock() else {
			return None;
		};
		let Some(lock3) = self.3.try_lock() else {
			return None;
		};
		let Some(lock4) = self.4.try_lock() else {
			return None;
		};

		Some((lock0, lock1, lock2, lock3, lock4))
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
	type Output = (
		A::Output,
		B::Output,
		C::Output,
		D::Output,
		E::Output,
		F::Output,
	);

	fn get_ptrs(&self) -> Vec<usize> {
		let mut ptrs = Vec::with_capacity(6);
		ptrs.append(&mut self.0.get_ptrs());
		ptrs.append(&mut self.1.get_ptrs());
		ptrs.append(&mut self.2.get_ptrs());
		ptrs.append(&mut self.3.get_ptrs());
		ptrs.append(&mut self.4.get_ptrs());
		ptrs.append(&mut self.5.get_ptrs());
		ptrs
	}

	unsafe fn lock(&'a self) -> Self::Output {
		loop {
			let lock0 = self.0.lock();
			let Some(lock1) = self.1.try_lock() else {
				continue;
			};
			let Some(lock2) = self.2.try_lock() else {
				continue;
			};
			let Some(lock3) = self.3.try_lock() else {
				continue;
			};
			let Some(lock4) = self.4.try_lock() else {
				continue;
			};
			let Some(lock5) = self.5.try_lock() else {
				continue;
			};

			return (lock0, lock1, lock2, lock3, lock4, lock5);
		}
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		let Some(lock0) = self.0.try_lock() else {
			return None;
		};
		let Some(lock1) = self.1.try_lock() else {
			return None;
		};
		let Some(lock2) = self.2.try_lock() else {
			return None;
		};
		let Some(lock3) = self.3.try_lock() else {
			return None;
		};
		let Some(lock4) = self.4.try_lock() else {
			return None;
		};
		let Some(lock5) = self.5.try_lock() else {
			return None;
		};

		Some((lock0, lock1, lock2, lock3, lock4, lock5))
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

unsafe impl<'a, T: Lockable<'a>, const N: usize> Lockable<'a> for [T; N] {
	type Output = [T::Output; N];

	fn get_ptrs(&self) -> Vec<usize> {
		let mut ptrs = Vec::with_capacity(N);
		for lock in self {
			ptrs.append(&mut lock.get_ptrs());
		}
		ptrs
	}

	unsafe fn lock(&'a self) -> Self::Output {
		unsafe fn unlock_partial<'a, T: Lockable<'a>, const N: usize>(
			guards: [MaybeUninit<T::Output>; N],
			upto: usize,
		) {
			for (i, guard) in guards.into_iter().enumerate() {
				if i == upto {
					break;
				}
				drop(guard.assume_init());
			}
		}

		let mut first_idx = 0;
		'outer: loop {
			let mut outputs = MaybeUninit::<[MaybeUninit<T::Output>; N]>::uninit().assume_init();
			if N == 0 {
				return outputs.map(|mu| mu.assume_init());
			}

			outputs[0].write(self[0].lock());
			for i in 0..N {
				if first_idx == i {
					continue;
				}

				match self[i].try_lock() {
					Some(guard) => outputs[i].write(guard),
					None => {
						unlock_partial::<T, N>(outputs, i);
						first_idx = i;
						continue 'outer;
					}
				};
			}

			return outputs.map(|mu| mu.assume_init());
		}
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		unsafe fn unlock_partial<'a, T: Lockable<'a>, const N: usize>(
			guards: [MaybeUninit<T::Output>; N],
			upto: usize,
		) {
			for (i, guard) in guards.into_iter().enumerate() {
				if i == upto {
					break;
				}
				drop(guard.assume_init());
			}
		}

		let mut outputs = MaybeUninit::<[MaybeUninit<T::Output>; N]>::uninit().assume_init();
		for i in 0..N {
			match self[i].try_lock() {
				Some(guard) => outputs[i].write(guard),
				None => {
					unlock_partial::<T, N>(outputs, i);
					return None;
				}
			};
		}

		Some(outputs.map(|mu| mu.assume_init()))
	}
}

unsafe impl<'a, T: Lockable<'a>> Lockable<'a> for Box<[T]> {
	type Output = Box<[T::Output]>;

	fn get_ptrs(&self) -> Vec<usize> {
		let mut ptrs = Vec::with_capacity(self.len());
		for lock in &**self {
			ptrs.append(&mut lock.get_ptrs());
		}
		ptrs
	}

	unsafe fn lock(&'a self) -> Self::Output {
		let mut first_idx = 0;
		if self.is_empty() {
			return Box::new([]);
		}

		'outer: loop {
			let mut outputs = Vec::with_capacity(self.len());

			outputs.push(self[first_idx].lock());
			for (idx, lock) in self.iter().enumerate() {
				if first_idx == idx {
					continue;
				}

				match lock.try_lock() {
					Some(guard) => {
						outputs.push(guard);
					}
					None => {
						first_idx = idx;
						continue 'outer;
					}
				};
			}

			return outputs.into_boxed_slice();
		}
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		let mut outputs = Vec::with_capacity(self.len());
		for lock in &**self {
			match lock.try_lock() {
				Some(guard) => {
					outputs.push(guard);
				}
				None => return None,
			};
		}

		Some(outputs.into_boxed_slice())
	}
}

unsafe impl<'a, T: Lockable<'a>> Lockable<'a> for Vec<T> {
	type Output = Vec<T::Output>;

	fn get_ptrs(&self) -> Vec<usize> {
		let mut ptrs = Vec::with_capacity(self.len());
		for lock in self {
			ptrs.append(&mut lock.get_ptrs());
		}
		ptrs
	}

	unsafe fn lock(&'a self) -> Self::Output {
		let mut first_idx = 0;
		if self.is_empty() {
			return Vec::new();
		}

		'outer: loop {
			let mut outputs = Vec::with_capacity(self.len());

			outputs.push(self[first_idx].lock());
			for (idx, lock) in self.iter().enumerate() {
				if first_idx == idx {
					continue;
				}

				match lock.try_lock() {
					Some(guard) => {
						outputs.push(guard);
					}
					None => {
						first_idx = idx;
						continue 'outer;
					}
				};
			}

			return outputs;
		}
	}

	unsafe fn try_lock(&'a self) -> Option<Self::Output> {
		let mut outputs = Vec::with_capacity(self.len());
		for lock in self {
			match lock.try_lock() {
				Some(guard) => {
					outputs.push(guard);
				}
				None => return None,
			};
		}

		Some(outputs)
	}
}

unsafe impl<'a, T: OwnedLockable<'a>, const N: usize> OwnedLockable<'a> for [T; N] {}
unsafe impl<'a, T: OwnedLockable<'a>> OwnedLockable<'a> for Box<[T]> {}
unsafe impl<'a, T: OwnedLockable<'a>> OwnedLockable<'a> for Vec<T> {}
