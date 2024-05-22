use crate::collection::{LockCollection, LockGuard, Readonly};
use crate::{Keyable, Sharable};

impl<Collection> Readonly<Collection> {
	pub fn new<L: Sharable>(data: L) -> Self
	where
		Collection: LockCollection<L>,
	{
		Self {
			collection: unsafe { Collection::new_readonly(data) },
		}
	}

	pub fn read<'g, 'key, L: Sharable, Key: Keyable + 'key>(
		&self,
		key: Key,
	) -> LockGuard<'key, L::ReadGuard<'g>, Key>
	where
		Collection: LockCollection<L>,
	{
		self.collection.read(key)
	}

	pub fn try_read<'g, 'key, L: Sharable, Key: Keyable + 'key>(
		&self,
		key: Key,
	) -> Option<LockGuard<'key, L::ReadGuard<'g>, Key>>
	where
		Collection: LockCollection<L>,
	{
		self.collection.try_read(key)
	}

	pub fn unlock_read<'key, L: Sharable, Key: Keyable + 'key>(
		guard: LockGuard<'key, L::ReadGuard<'_>, Key>,
	) -> Key
	where
		Collection: LockCollection<L>,
	{
		Collection::unlock_read(guard)
	}
}
