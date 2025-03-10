use std::cell::Cell;

use crate::handle_unwind::handle_unwind;
use crate::lockable::{Lockable, RawLock, Sharable};
use crate::Keyable;

#[must_use]
pub fn get_locks<L: Lockable>(data: &L) -> Vec<&dyn RawLock> {
	let mut locks = Vec::new();
	data.get_ptrs(&mut locks);
	locks.sort_by_key(|lock| &raw const **lock);
	locks
}

#[must_use]
pub fn get_locks_unsorted<L: Lockable>(data: &L) -> Vec<&dyn RawLock> {
	let mut locks = Vec::new();
	data.get_ptrs(&mut locks);
	locks
}

/// returns `true` if the sorted list contains a duplicate
#[must_use]
pub fn ordered_contains_duplicates(l: &[&dyn RawLock]) -> bool {
	if l.is_empty() {
		// Return early to prevent panic in the below call to `windows`
		return false;
	}

	l.windows(2)
		// NOTE: addr_eq is necessary because eq would also compare the v-table pointers
		.any(|window| std::ptr::addr_eq(window[0], window[1]))
}

/// Lock a set of locks in the given order. It's UB to call this without a `ThreadKey`
pub unsafe fn ordered_write(locks: &[&dyn RawLock]) {
	// these will be unlocked in case of a panic
	let locked = Cell::new(0);

	handle_unwind(
		|| {
			for lock in locks {
				lock.raw_write();
				locked.set(locked.get() + 1);
			}
		},
		|| attempt_to_recover_writes_from_panic(&locks[0..locked.get()]),
	)
}

/// Lock a set of locks in the given order. It's UB to call this without a `ThreadKey`
pub unsafe fn ordered_read(locks: &[&dyn RawLock]) {
	let locked = Cell::new(0);

	handle_unwind(
		|| {
			for lock in locks {
				lock.raw_read();
				locked.set(locked.get() + 1);
			}
		},
		|| attempt_to_recover_reads_from_panic(&locks[0..locked.get()]),
	)
}

/// Locks the locks in the order they are given. This causes deadlock if the
/// locks contain duplicates, or if this is called by multiple threads with the
/// locks in different orders.
pub unsafe fn ordered_try_write(locks: &[&dyn RawLock]) -> bool {
	let locked = Cell::new(0);

	handle_unwind(
		|| unsafe {
			for (i, lock) in locks.iter().enumerate() {
				// safety: we have the thread key
				if lock.raw_try_write() {
					locked.set(locked.get() + 1);
				} else {
					for lock in &locks[0..i] {
						// safety: this lock was already acquired
						lock.raw_unlock_write();
					}
					return false;
				}
			}

			true
		},
		||
		// safety: everything in locked is locked
		attempt_to_recover_writes_from_panic(&locks[0..locked.get()]),
	)
}

/// Locks the locks in the order they are given. This causes deadlock if this
/// is called by multiple threads with the locks in different orders.
pub unsafe fn ordered_try_read(locks: &[&dyn RawLock]) -> bool {
	// these will be unlocked in case of a panic
	let locked = Cell::new(0);

	handle_unwind(
		|| unsafe {
			for (i, lock) in locks.iter().enumerate() {
				// safety: we have the thread key
				if lock.raw_try_read() {
					locked.set(locked.get() + 1);
				} else {
					for lock in &locks[0..i] {
						// safety: this lock was already acquired
						lock.raw_unlock_read();
					}
					return false;
				}
			}

			true
		},
		||
		// safety: everything in locked is locked
		attempt_to_recover_reads_from_panic(&locks[0..locked.get()]),
	)
}

pub fn scoped_write<'a, L: RawLock + Lockable, R>(
	collection: &'a L,
	key: impl Keyable,
	f: impl FnOnce(L::DataMut<'a>) -> R,
) -> R {
	unsafe {
		// safety: we have the key
		collection.raw_write();

		// safety: we just locked this
		let r = f(collection.data_mut());

		// this ensures the key is held long enough
		drop(key);

		// safety: we've locked already, and aren't using the data again
		collection.raw_unlock_write();

		r
	}
}

pub fn scoped_try_write<'a, L: RawLock + Lockable, Key: Keyable, R>(
	collection: &'a L,
	key: Key,
	f: impl FnOnce(L::DataMut<'a>) -> R,
) -> Result<R, Key> {
	unsafe {
		// safety: we have the key
		if !collection.raw_try_write() {
			return Err(key);
		}

		// safety: we just locked this
		let r = f(collection.data_mut());

		// this ensures the key is held long enough
		drop(key);

		// safety: we've locked already, and aren't using the data again
		collection.raw_unlock_write();

		Ok(r)
	}
}

pub fn scoped_read<'a, L: RawLock + Sharable, R>(
	collection: &'a L,
	key: impl Keyable,
	f: impl FnOnce(L::DataRef<'a>) -> R,
) -> R {
	unsafe {
		// safety: we have the key
		collection.raw_read();

		// safety: we just locked this
		let r = f(collection.data_ref());

		// this ensures the key is held long enough
		drop(key);

		// safety: we've locked already, and aren't using the data again
		collection.raw_unlock_read();

		r
	}
}

pub fn scoped_try_read<'a, L: RawLock + Sharable, Key: Keyable, R>(
	collection: &'a L,
	key: Key,
	f: impl FnOnce(L::DataRef<'a>) -> R,
) -> Result<R, Key> {
	unsafe {
		// safety: we have the key
		if !collection.raw_try_read() {
			return Err(key);
		}

		// safety: we just locked this
		let r = f(collection.data_ref());

		// this ensures the key is held long enough
		drop(key);

		// safety: we've locked already, and aren't using the data again
		collection.raw_unlock_read();

		Ok(r)
	}
}

/// Unlocks the already locked locks in order to recover from a panic
pub unsafe fn attempt_to_recover_writes_from_panic(locks: &[&dyn RawLock]) {
	handle_unwind(
		|| {
			// safety: the caller assumes that these are already locked
			locks.iter().for_each(|lock| lock.raw_unlock_write());
		},
		// if we get another panic in here, we'll just have to poison what remains
		|| locks.iter().for_each(|l| l.poison()),
	)
}

/// Unlocks the already locked locks in order to recover from a panic
pub unsafe fn attempt_to_recover_reads_from_panic(locked: &[&dyn RawLock]) {
	handle_unwind(
		|| {
			// safety: the caller assumes these are already locked
			locked.iter().for_each(|lock| lock.raw_unlock_read());
		},
		// if we get another panic in here, we'll just have to poison what remains
		|| locked.iter().for_each(|l| l.poison()),
	)
}

#[cfg(test)]
mod tests {
	use crate::collection::utils::ordered_contains_duplicates;

	#[test]
	fn empty_array_does_not_contain_duplicates() {
		assert!(!ordered_contains_duplicates(&[]))
	}
}
