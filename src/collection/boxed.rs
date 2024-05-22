use std::fmt::Debug;
use std::marker::PhantomData;

use crate::lockable::Lock;
use crate::{Keyable, Lockable, OwnedLockable, Sharable};

use super::{BoxedLockCollection, LockGuard};

/// returns `true` if the sorted list contains a duplicate
#[must_use]
fn contains_duplicates(l: &[&dyn Lock]) -> bool {
	l.windows(2)
		.any(|window| std::ptr::eq(window[0], window[1]))
}

unsafe impl<L: Lockable> Lockable for BoxedLockCollection<L> {
	type Guard<'g> = L::Guard<'g> where Self: 'g;

	type ReadGuard<'g> = L::ReadGuard<'g> where Self: 'g;

	fn get_ptrs<'a>(&'a self, ptrs: &mut Vec<&'a dyn Lock>) {
		self.data.get_ptrs(ptrs)
	}

	unsafe fn guard(&self) -> Self::Guard<'_> {
		self.data.guard()
	}

	unsafe fn read_guard(&self) -> Self::ReadGuard<'_> {
		self.data.read_guard()
	}
}

unsafe impl<L: Sharable> Sharable for BoxedLockCollection<L> {}

unsafe impl<L: OwnedLockable> OwnedLockable for BoxedLockCollection<L> {}

impl<L> IntoIterator for BoxedLockCollection<L>
where
	L: IntoIterator,
{
	type Item = <L as IntoIterator>::Item;
	type IntoIter = <L as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

impl<'a, L> IntoIterator for &'a BoxedLockCollection<L>
where
	&'a L: IntoIterator,
{
	type Item = <&'a L as IntoIterator>::Item;
	type IntoIter = <&'a L as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

impl<'a, L> IntoIterator for &'a mut BoxedLockCollection<L>
where
	&'a mut L: IntoIterator,
{
	type Item = <&'a mut L as IntoIterator>::Item;
	type IntoIter = <&'a mut L as IntoIterator>::IntoIter;

	fn into_iter(self) -> Self::IntoIter {
		self.data.into_iter()
	}
}

impl<L: OwnedLockable, I: FromIterator<L> + OwnedLockable> FromIterator<L>
	for BoxedLockCollection<I>
{
	fn from_iter<T: IntoIterator<Item = L>>(iter: T) -> Self {
		let iter: I = iter.into_iter().collect();
		Self::new(iter)
	}
}

impl<E: OwnedLockable + Extend<L>, L: OwnedLockable> Extend<L> for BoxedLockCollection<E> {
	fn extend<T: IntoIterator<Item = L>>(&mut self, iter: T) {
		self.data.extend(iter)
	}
}

impl<L> AsRef<L> for BoxedLockCollection<L> {
	fn as_ref(&self) -> &L {
		&self.data
	}
}

impl<L> AsMut<L> for BoxedLockCollection<L> {
	fn as_mut(&mut self) -> &mut L {
		&mut self.data
	}
}

impl<L: Debug> Debug for BoxedLockCollection<L> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct(stringify!(BoxedLockCollection))
			.field("data", &self.data)
			.finish_non_exhaustive()
	}
}

impl<L: OwnedLockable + Default> Default for BoxedLockCollection<L> {
	fn default() -> Self {
		Self::new(L::default())
	}
}

impl<L: OwnedLockable + Default> From<L> for BoxedLockCollection<L> {
	fn from(value: L) -> Self {
		Self::new(value)
	}
}

impl<L: OwnedLockable> BoxedLockCollection<L> {
	#[must_use]
	pub fn new(data: L) -> Self {
		// safety: owned lockable types cannot contain duplicates
		unsafe { Self::new_unchecked(data) }
	}
}

impl<'a, L: OwnedLockable> BoxedLockCollection<&'a L> {
	#[must_use]
	pub fn new_ref(data: &'a L) -> Self {
		// safety: owned lockable types cannot contain duplicates
		unsafe { Self::new_unchecked(data) }
	}
}

impl<L: Lockable> BoxedLockCollection<L> {
	#[must_use]
	pub unsafe fn new_unchecked(data: L) -> Self {
		let data = Box::new(data);
		let mut locks = Vec::new();
		data.get_ptrs(&mut locks);

		// safety: the box will be dropped after the lock references, so it's
		//         safe to just pretend they're static
		let locks = std::mem::transmute(locks);
		Self { data, locks }
	}

	#[must_use]
	pub fn try_new(data: L) -> Option<Self> {
		// safety: we are checking for duplicates before returning
		unsafe {
			let this = Self::new_unchecked(data);
			if contains_duplicates(&this.locks) {
				return None;
			}
			Some(this)
		}
	}

	#[must_use]
	pub fn into_inner(self) -> Box<L> {
		self.data
	}

	pub fn lock<'g, 'key: 'g, Key: Keyable + 'key>(
		&'g self,
		key: Key,
	) -> LockGuard<'key, L::Guard<'g>, Key> {
		for lock in &self.locks {
			// safety: we have the thread key
			unsafe { lock.lock() };
		}

		LockGuard {
			// safety: we've already acquired the lock
			guard: unsafe { self.data.guard() },
			key,
			_phantom: PhantomData,
		}
	}

	pub fn try_lock<'g, 'key: 'g, Key: Keyable + 'key>(
		&'g self,
		key: Key,
	) -> Option<LockGuard<'key, L::Guard<'g>, Key>> {
		let guard = unsafe {
			for (i, lock) in self.locks.iter().enumerate() {
				// safety: we have the thread key
				let success = lock.try_lock();

				if !success {
					for lock in &self.locks[0..i] {
						// safety: this lock was already acquired
						lock.unlock();
					}
					return None;
				}
			}

			// safety: we've acquired the locks
			self.data.guard()
		};

		Some(LockGuard {
			guard,
			key,
			_phantom: PhantomData,
		})
	}

	pub fn unlock<'key, Key: Keyable + 'key>(guard: LockGuard<'key, L::Guard<'_>, Key>) -> Key {
		drop(guard.guard);
		guard.key
	}
}

impl<L: Sharable> BoxedLockCollection<L> {
	pub fn read<'g, 'key: 'g, Key: Keyable + 'key>(
		&'g self,
		key: Key,
	) -> LockGuard<'key, L::ReadGuard<'g>, Key> {
		for lock in &self.locks {
			// safety: we have the thread key
			unsafe { lock.read() };
		}

		LockGuard {
			// safety: we've already acquired the lock
			guard: unsafe { self.data.read_guard() },
			key,
			_phantom: PhantomData,
		}
	}

	pub fn try_read<'g, 'key: 'g, Key: Keyable + 'key>(
		&'g self,
		key: Key,
	) -> Option<LockGuard<'key, L::ReadGuard<'g>, Key>> {
		let guard = unsafe {
			for (i, lock) in self.locks.iter().enumerate() {
				// safety: we have the thread key
				let success = lock.try_read();

				if !success {
					for lock in &self.locks[0..i] {
						// safety: this lock was already acquired
						lock.unlock_read();
					}
					return None;
				}
			}

			// safety: we've acquired the locks
			self.data.read_guard()
		};

		Some(LockGuard {
			guard,
			key,
			_phantom: PhantomData,
		})
	}

	pub fn unlock_read<'key, Key: Keyable + 'key>(
		guard: LockGuard<'key, L::ReadGuard<'_>, Key>,
	) -> Key {
		drop(guard.guard);
		guard.key
	}
}

impl<'a, L: 'a> BoxedLockCollection<L>
where
	&'a L: IntoIterator,
{
	/// Returns an iterator over references to each value in the collection.
	#[must_use]
	pub fn iter(&'a self) -> <&'a L as IntoIterator>::IntoIter {
		self.into_iter()
	}
}

impl<'a, L: 'a> BoxedLockCollection<L>
where
	&'a mut L: IntoIterator,
{
	/// Returns an iterator over mutable references to each value in the
	/// collection.
	#[must_use]
	pub fn iter_mut(&'a mut self) -> <&'a mut L as IntoIterator>::IntoIter {
		self.into_iter()
	}
}
