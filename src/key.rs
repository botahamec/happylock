use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};

use once_cell::sync::Lazy;
use thread_local::ThreadLocal;

use sealed::Sealed;

// Sealed to prevent other key types from being implemented. Otherwise, this
// would almost instant undefined behavior.
mod sealed {
	use super::ThreadKey;

	pub trait Sealed {}
	impl Sealed for ThreadKey {}
	impl Sealed for &mut ThreadKey {}
}

// I am concerned that having multiple crates linked together with different
// static variables could break my key system. Library code probably shouldn't
// be creating keys at all.
static KEY: Lazy<ThreadLocal<AtomicLock>> = Lazy::new(ThreadLocal::new);

/// The key for the current thread.
///
/// Only one of these exist per thread. To get the current thread's key, call
/// [`ThreadKey::get`]. If the `ThreadKey` is dropped, it can be re-obtained.
pub struct ThreadKey {
	phantom: PhantomData<*const ()>, // implement !Send and !Sync
}

/// Allows the type to be used as a key for a lock
///
/// # Safety
///
/// Only one value which implements this trait may be allowed to exist at a
/// time. Creating a new `Keyable` value requires making any other `Keyable`
/// values invalid.
pub unsafe trait Keyable: Sealed {}
unsafe impl Keyable for ThreadKey {}
// the ThreadKey can't be moved while a mutable reference to it exists
unsafe impl Keyable for &mut ThreadKey {}

impl Debug for ThreadKey {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "ThreadKey")
	}
}

// If you lose the thread key, you can get it back by calling ThreadKey::get
impl Drop for ThreadKey {
	fn drop(&mut self) {
		unsafe { KEY.get().unwrap().force_unlock() }
	}
}

impl ThreadKey {
	/// Get the current thread's `ThreadKey`, if it's not already taken.
	///
	/// The first time this is called, it will successfully return a
	/// `ThreadKey`. However, future calls to this function on the same thread
	/// will return [`None`], unless the key is dropped or unlocked first.
	///
	/// # Examples
	///
	/// ```
	/// use happylock::ThreadKey;
	///
	/// let key = ThreadKey::get().unwrap();
	/// ```
	#[must_use]
	pub fn get() -> Option<Self> {
		// safety: we just acquired the lock
		KEY.get_or_default().try_lock().then_some(Self {
			phantom: PhantomData,
		})
	}
}

/// A dumb lock that's just a wrapper for an [`AtomicBool`].
#[derive(Debug, Default)]
struct AtomicLock {
	is_locked: AtomicBool,
}

impl AtomicLock {
	/// Attempt to lock the `AtomicLock`. This is not a fair lock.
	#[must_use]
	pub fn try_lock(&'static self) -> bool {
		!self.is_locked.swap(true, Ordering::Acquire)
	}

	/// Forcibly unlocks the `AtomicLock`. This should only be called if the
	/// key to the lock has been "lost".
	pub unsafe fn force_unlock(&self) {
		self.is_locked.store(false, Ordering::Release);
	}
}
