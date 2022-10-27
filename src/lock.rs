use std::sync::atomic::{AtomicBool, Ordering};

/// A dumb lock that's just a wrapper for an [`AtomicBool`].
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
		!self.is_locked.fetch_or(true, Ordering::Acquire)
	}

	pub fn unlock(&self) {
		self.is_locked.store(false, Ordering::Release)
	}
}
