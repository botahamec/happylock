use crate::lockable::RawLock;

/// Locks the locks in the order they are given. This causes deadlock if the
/// locks contain duplicates, or if this is called by multiple threads with the
/// locks in different orders.
pub unsafe fn ordered_try_lock(locks: &[&dyn RawLock]) -> bool {
	unsafe {
		for (i, lock) in locks.iter().enumerate() {
			// safety: we have the thread key
			let success = lock.try_lock();

			if !success {
				for lock in &locks[0..i] {
					// safety: this lock was already acquired
					lock.unlock();
				}
				return false;
			}
		}

		true
	}
}

/// Locks the locks in the order they are given. This causes deadlock f this is
/// called by multiple threads with the locks in different orders.
pub unsafe fn ordered_try_read(locks: &[&dyn RawLock]) -> bool {
	unsafe {
		for (i, lock) in locks.iter().enumerate() {
			// safety: we have the thread key
			let success = lock.try_read();

			if !success {
				for lock in &locks[0..i] {
					// safety: this lock was already acquired
					lock.unlock_read();
				}
				return false;
			}
		}

		true
	}
}
