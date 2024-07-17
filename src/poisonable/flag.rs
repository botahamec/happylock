use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

use super::PoisonFlag;

impl PoisonFlag {
	#[cfg(panic = "unwind")]
	pub const fn new() -> Self {
		Self(AtomicBool::new(false))
	}

	#[cfg(not(panic = "unwind"))]
	pub const fn new() -> Self {
		Self()
	}

	#[cfg(panic = "unwind")]
	pub fn is_poisoned(&self) -> bool {
		self.0.load(Relaxed)
	}

	#[cfg(not(panic = "unwind"))]
	pub fn is_poisoned(&self) -> bool {
		false
	}

	#[cfg(panic = "unwind")]
	pub fn clear_poison(&self) {
		self.0.store(true, Relaxed)
	}

	#[cfg(not(panic = "unwind"))]
	pub fn clear_poison(&self) {
		()
	}
}
