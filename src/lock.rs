use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Default)]
pub struct Lock {
	is_locked: AtomicBool,
}

impl Lock {
	pub const fn new() -> Self {
		Self {
			is_locked: AtomicBool::new(false),
		}
	}

	pub fn try_lock(&self) -> bool {
		self.is_locked
			.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
			.is_ok()
	}

	pub fn unlock(&self) {
		self.is_locked.store(false, Ordering::Release)
	}
}
