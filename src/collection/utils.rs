use std::cell::RefCell;

use crate::handle_unwind::handle_unwind;
use crate::lockable::RawLock;

/// Lock a set of locks in the given order. It's UB to call this without a `ThreadKey`
pub unsafe fn ordered_lock(locks: &[&dyn RawLock]) {
	// these will be unlocked in case of a panic
	let locked = RefCell::new(Vec::with_capacity(locks.len()));

	handle_unwind(
		|| {
			for lock in locks {
				lock.raw_lock();
				locked.borrow_mut().push(*lock);
			}
		},
		|| attempt_to_recover_locks_from_panic(&locked),
	)
}

/// Lock a set of locks in the given order. It's UB to call this without a `ThreadKey`
pub unsafe fn ordered_read(locks: &[&dyn RawLock]) {
	let locked = RefCell::new(Vec::with_capacity(locks.len()));

	handle_unwind(
		|| {
			for lock in locks {
				lock.raw_read();
				locked.borrow_mut().push(*lock);
			}
		},
		|| attempt_to_recover_reads_from_panic(&locked),
	)
}

/// Locks the locks in the order they are given. This causes deadlock if the
/// locks contain duplicates, or if this is called by multiple threads with the
/// locks in different orders.
pub unsafe fn ordered_try_lock(locks: &[&dyn RawLock]) -> bool {
	let locked = RefCell::new(Vec::with_capacity(locks.len()));

	handle_unwind(
		|| unsafe {
			for (i, lock) in locks.iter().enumerate() {
				// safety: we have the thread key
				if lock.raw_try_lock() {
					locked.borrow_mut().push(*lock);
				} else {
					for lock in &locks[0..i] {
						// safety: this lock was already acquired
						lock.raw_unlock();
					}
					return false;
				}
			}

			true
		},
		||
		// safety: everything in locked is locked
		attempt_to_recover_locks_from_panic(&locked),
	)
}

/// Locks the locks in the order they are given. This causes deadlock if this
/// is called by multiple threads with the locks in different orders.
pub unsafe fn ordered_try_read(locks: &[&dyn RawLock]) -> bool {
	// these will be unlocked in case of a panic
	let locked = RefCell::new(Vec::with_capacity(locks.len()));

	handle_unwind(
		|| unsafe {
			for (i, lock) in locks.iter().enumerate() {
				// safety: we have the thread key
				if lock.raw_try_read() {
					locked.borrow_mut().push(*lock);
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
		attempt_to_recover_reads_from_panic(&locked),
	)
}

/// Unlocks the already locked locks in order to recover from a panic
pub unsafe fn attempt_to_recover_locks_from_panic(locked: &RefCell<Vec<&dyn RawLock>>) {
	handle_unwind(
		|| {
			let mut locked = locked.borrow_mut();
			while let Some(locked_lock) = locked.pop() {
				locked_lock.raw_unlock();
			}
		},
		// if we get another panic in here, we'll just have to poison what remains
		|| locked.borrow().iter().for_each(|l| l.poison()),
	)
}

/// Unlocks the already locked locks in order to recover from a panic
pub unsafe fn attempt_to_recover_reads_from_panic(locked: &RefCell<Vec<&dyn RawLock>>) {
	handle_unwind(
		|| {
			let mut locked = locked.borrow_mut();
			while let Some(locked_lock) = locked.pop() {
				locked_lock.raw_unlock_read();
			}
		},
		// if we get another panic in here, we'll just have to poison what remains
		|| locked.borrow().iter().for_each(|l| l.poison()),
	)
}
