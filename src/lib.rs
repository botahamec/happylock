use std::any::type_name;
use std::fmt::{self, Debug};

use parking_lot::Mutex;

thread_local! {
	// safety: this is the only place where a ThreadLock is created
	pub static KEY: Mutex<Option<ThreadKey>> = Mutex::new(Some(unsafe { ThreadKey::new() }));
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
		KEY.with(|thread_lock| {
			let mut key = thread_lock.lock();

			// safety: this ThreadKey is about to be destroyed, so there'll
			//         still only be one ThreadKey
			*key = Some(unsafe { Self::new() });
		})
	}
}

impl ThreadKey {
	/// Create a new `ThreadKey`.
	///
	/// # Safety
	///
	/// There should only be one `ThreadKey` per thread.
	unsafe fn new() -> Self {
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
		KEY.with(|thread_lock| thread_lock.lock().take())
	}

	/// Unlocks the `ThreadKey`.
	///
	/// After this method is called, a call to [`ThreadKey::lock`] will return
	/// this `ThreadKey`.
	pub fn unlock(key: ThreadKey) {
		drop(key)
	}
}
