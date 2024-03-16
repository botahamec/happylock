use std::ops::{Deref, DerefMut};

use crate::{Lockable, OwnedLockable};

use super::{BoxedLockCollection, RefLockCollection};

impl<'a, L> Deref for BoxedLockCollection<'a, L> {
	type Target = RefLockCollection<'a, L>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<'a, L> DerefMut for BoxedLockCollection<'a, L> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl<'a, L> Drop for BoxedLockCollection<'a, L> {
	fn drop(&mut self) {
		// this was created with Box::new
		let boxed = unsafe { Box::from_raw((self.0.data as *const L).cast_mut()) };
		drop(boxed);
	}
}

impl<'a, L: OwnedLockable> BoxedLockCollection<'a, L> {
	#[must_use]
	pub fn new(data: L) -> Self {
		let boxed = Box::leak(Box::new(data));
		Self(RefLockCollection::new(boxed))
	}
}

impl<'a, L: OwnedLockable> BoxedLockCollection<'a, &'a L> {
	#[must_use]
	pub fn new_ref(data: &'a L) -> Self {
		let boxed = Box::leak(Box::new(data));

		// safety: this is a reference to an OwnedLockable, which can't
		//         possibly contain inner duplicates
		Self(unsafe { RefLockCollection::new_unchecked(boxed) })
	}
}

impl<'a, L: Lockable> BoxedLockCollection<'a, L> {
	#[must_use]
	pub unsafe fn new_unchecked(data: L) -> Self {
		let boxed = Box::leak(Box::new(data));
		Self(RefLockCollection::new_unchecked(boxed))
	}

	#[must_use]
	pub fn try_new(data: L) -> Option<Self> {
		let boxed = Box::leak(Box::new(data));
		RefLockCollection::try_new(boxed).map(Self)
	}
}
