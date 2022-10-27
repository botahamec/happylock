use std::any::type_name;
use std::fmt::{self, Debug};

mod lock;

pub use lock::Lock;

thread_local! {
	pub static KEY: Lock = Lock::new();
}

/// The key for the current thread.
///
/// Only one of these exist per thread. To get the current thread's key, call
/// [`ThreadKey::lock`].
pub struct ThreadKey {
	_priv: *const (), // this isn't Send or Sync
}

impl Debug for ThreadKey {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		type_name::<Self>().fmt(f)
	}
}

impl Drop for ThreadKey {
	fn drop(&mut self) {
		KEY.with(|lock| lock.unlock());
	}
}

impl ThreadKey {
	/// Create a new `ThreadKey`. There should only be one `ThreadKey` per thread.
	fn new() -> Self {
		Self {
			_priv: std::ptr::null(),
		}
	}

	/// Get the current thread's `ThreadKey, if it's not already taken.
	///
	/// The first time this is called, it will successfully return a
	/// `ThreadKey`. However, future calls to this function will return
	/// [`None`], unless the key is dropped or unlocked first.
	pub fn lock() -> Option<Self> {
		KEY.with(|lock| lock.try_lock().then_some(Self::new()))
	}

	/// Unlocks the `ThreadKey`.
	///
	/// After this method is called, a call to [`ThreadKey::lock`] will return
	/// this `ThreadKey`.
	pub fn unlock(key: ThreadKey) {
		drop(key)
	}
}
