use std::cell::RefCell;

use crate::lockable::RawLock;

/// Locks the locks in the order they are given. This causes deadlock if the
/// locks contain duplicates, or if this is called by multiple threads with the
/// locks in different orders.
pub unsafe fn ordered_try_lock(locks: &[&dyn RawLock]) -> bool {
	let locked = RefCell::new(Vec::with_capacity(locks.len()));
	scopeguard::defer_on_unwind! {
		// safety: everything in locked is locked
		attempt_to_recover_locks_from_panic(&locked)
	};

	unsafe {
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
	}
}

/// Locks the locks in the order they are given. This causes deadlock if this
/// is called by multiple threads with the locks in different orders.
pub unsafe fn ordered_try_read(locks: &[&dyn RawLock]) -> bool {
	let locked = RefCell::new(Vec::with_capacity(locks.len()));
	scopeguard::defer_on_unwind! {
		// safety: everything in locked is locked
		attempt_to_recover_reads_from_panic(&locked)
	};

	unsafe {
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
	}
}

pub unsafe fn attempt_to_recover_locks_from_panic(locked: &RefCell<Vec<&dyn RawLock>>) {
	scopeguard::defer_on_unwind! { locked.borrow().iter().for_each(|l| l.kill()); };
	let mut locked = locked.borrow_mut();
	while let Some(locked_lock) = locked.pop() {
		locked_lock.raw_unlock();
	}
}

pub unsafe fn attempt_to_recover_reads_from_panic(locked: &RefCell<Vec<&dyn RawLock>>) {
	scopeguard::defer_on_unwind! { locked.borrow().iter().for_each(|l| l.kill()); };
	let mut locked = locked.borrow_mut();
	while let Some(locked_lock) = locked.pop() {
		locked_lock.raw_unlock_read();
	}
}
