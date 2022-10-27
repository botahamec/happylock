#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]

use std::any::type_name;
use std::fmt::{self, Debug};
use std::marker::PhantomData;

use once_cell::sync::Lazy;
use thread_local::ThreadLocal;

mod lock;

use lock::{Key, Lock};

static KEY: Lazy<ThreadLocal<Lock>> = Lazy::new(ThreadLocal::new);

/// The key for the current thread.
///
/// Only one of these exist per thread. To get the current thread's key, call
/// [`ThreadKey::lock`].
pub struct ThreadKey {
	phantom: PhantomData<*const ()>, // implement !Send and !Sync
	_key: Key<'static>,
}

impl Debug for ThreadKey {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		type_name::<Self>().fmt(f)
	}
}

impl ThreadKey {
	/// Get the current thread's `ThreadKey`, if it's not already taken.
	///
	/// The first time this is called, it will successfully return a
	/// `ThreadKey`. However, future calls to this function will return
	/// [`None`], unless the key is dropped or unlocked first.
	#[must_use]
	pub fn lock() -> Option<Self> {
		KEY.get_or_default().try_lock().map(|key| Self {
			phantom: PhantomData,
			_key: key,
		})
	}

	/// Unlocks the `ThreadKey`.
	///
	/// After this method is called, a call to [`ThreadKey::lock`] will return
	/// this `ThreadKey`.
	pub fn unlock(key: Self) {
		drop(key);
	}

	/// Unlocks the `ThreadKey` without consuming it.
	///
	/// # Safety
	///
	/// This should only be called if the `ThreadKey` to the lock has been
	/// "lost". That means the program no longer has a reference to the key,
	/// but it has not been dropped.
	pub unsafe fn force_unlock() {
		KEY.get_or_default().force_unlock();
	}
}
