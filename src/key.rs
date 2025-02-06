use std::cell::{Cell, LazyCell};
use std::fmt::{self, Debug};
use std::marker::PhantomData;

use sealed::Sealed;

// Sealed to prevent other key types from being implemented. Otherwise, this
// would almost instant undefined behavior.
mod sealed {
	use super::ThreadKey;

	pub trait Sealed {}
	impl Sealed for ThreadKey {}
	impl Sealed for &mut ThreadKey {}
}

thread_local! {
	static KEY: LazyCell<KeyCell> = LazyCell::new(KeyCell::default);
}

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

// Implementing this means we can allow `MutexGuard` to be Sync
// Safety: a &ThreadKey is useless by design.
unsafe impl Sync for ThreadKey {}

#[mutants::skip]
#[cfg(not(tarpaulin_include))]
impl Debug for ThreadKey {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "ThreadKey")
	}
}

// If you lose the thread key, you can get it back by calling ThreadKey::get
impl Drop for ThreadKey {
	fn drop(&mut self) {
		// safety: a thread key cannot be acquired without creating the lock
		// safety: the key is lost, so it's safe to unlock the cell
		unsafe { KEY.with(|key| key.force_unlock()) }
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
		// safety: if this code changes, check to ensure the requirement for
		//         the Drop implementation is still true
		KEY.with(|key| {
			key.try_lock().then_some(Self {
				phantom: PhantomData,
			})
		})
	}
}

/// A dumb lock that's just a wrapper for an [`AtomicBool`].
#[derive(Default)]
struct KeyCell {
	is_locked: Cell<bool>,
}

impl KeyCell {
	/// Attempt to lock the `KeyCell`. This is not a fair lock.
	#[must_use]
	pub fn try_lock(&self) -> bool {
		!self.is_locked.replace(true)
	}

	/// Forcibly unlocks the `KeyCell`. This should only be called if the key
	/// from this `KeyCell` has been "lost".
	pub unsafe fn force_unlock(&self) {
		self.is_locked.set(false);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn thread_key_returns_some_on_first_call() {
		assert!(ThreadKey::get().is_some());
	}

	#[test]
	fn thread_key_returns_none_on_second_call() {
		let key = ThreadKey::get();
		assert!(ThreadKey::get().is_none());
		drop(key);
	}

	#[test]
	fn dropping_thread_key_allows_reobtaining() {
		drop(ThreadKey::get());
		assert!(ThreadKey::get().is_some())
	}
}
