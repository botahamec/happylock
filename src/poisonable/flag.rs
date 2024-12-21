#[cfg(panic = "unwind")]
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

use super::PoisonFlag;

#[cfg(panic = "unwind")]
impl PoisonFlag {
	pub const fn new() -> Self {
		Self(AtomicBool::new(false))
	}

	pub fn is_poisoned(&self) -> bool {
		self.0.load(Relaxed)
	}

	pub fn clear_poison(&self) {
		self.0.store(false, Relaxed)
	}

	pub fn poison(&self) {
		self.0.store(true, Relaxed);
	}
}

#[cfg(not(panic = "unwind"))]
impl PoisonFlag {
	pub const fn new() -> Self {
		Self()
	}

	pub fn is_poisoned(&self) -> bool {
		false
	}

	pub fn clear_poison(&self) {
		()
	}
}
