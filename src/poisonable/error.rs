use core::fmt;
use std::error::Error;

use super::{PoisonError, PoisonGuard, TryLockPoisonableError};

impl<Guard> fmt::Debug for PoisonError<Guard> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("PoisonError").finish_non_exhaustive()
	}
}

impl<Guard> fmt::Display for PoisonError<Guard> {
	#[cfg_attr(test, mutants::skip)]
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		"poisoned lock: another task failed inside".fmt(f)
	}
}

impl<Guard> AsRef<Guard> for PoisonError<Guard> {
	fn as_ref(&self) -> &Guard {
		self.get_ref()
	}
}

impl<Guard> AsMut<Guard> for PoisonError<Guard> {
	fn as_mut(&mut self) -> &mut Guard {
		self.get_mut()
	}
}

impl<Guard> Error for PoisonError<Guard> {}

impl<Guard> PoisonError<Guard> {
	/// Creates a `PoisonError`
	///
	/// This is generally created by methods like [`Poisonable::lock`].
	///
	/// ```
	/// use happylock::poisonable::PoisonError;
	///
	/// let error = PoisonError::new("oh no");
	/// ```
	///
	/// [`Poisonable::lock`]: `crate::poisonable::Poisonable::lock`
	#[must_use]
	pub const fn new(guard: Guard) -> Self {
		Self { guard }
	}

	/// Consumes the error indicating that a lock is poisonmed, returning the
	/// underlying guard to allow access regardless.
	///
	/// # Examples
	///
	/// ```
	/// use std::collections::HashSet;
	/// use std::sync::Arc;
	/// use std::thread;
	///
	/// use happylock::{Mutex, Poisonable, ThreadKey};
	///
	/// let mutex = Arc::new(Poisonable::new(Mutex::new(HashSet::new())));
	///
	/// // poison the mutex
	/// let c_mutex = Arc::clone(&mutex);
	/// let _ = thread::spawn(move || {
	///     let key = ThreadKey::get().unwrap();
	///     let mut data = c_mutex.lock(key).unwrap();
	///     data.insert(10);
	///     panic!();
	/// }).join();
	///
	/// let key = ThreadKey::get().unwrap();
	/// let p_err = mutex.lock(key).unwrap_err();
	/// let data = p_err.into_inner();
	/// println!("recovered {} items", data.len());
	/// ```
	#[must_use]
	pub fn into_inner(self) -> Guard {
		self.guard
	}

	/// Reaches into this error indicating that a lock is poisoned, returning a
	/// reference to the underlying guard to allow access regardless.
	///
	/// # Examples
	///
	/// ```
	/// use std::collections::HashSet;
	/// use std::sync::Arc;
	/// use std::thread;
	///
	/// use happylock::{Mutex, Poisonable, ThreadKey};
	/// use happylock::poisonable::PoisonGuard;
	///
	/// let mutex = Arc::new(Poisonable::new(Mutex::new(HashSet::new())));
	///
	/// // poison the mutex
	/// let c_mutex = Arc::clone(&mutex);
	/// let _ = thread::spawn(move || {
	///     let key = ThreadKey::get().unwrap();
	///     let mut data = c_mutex.lock(key).unwrap();
	///     data.insert(10);
	///     panic!();
	/// }).join();
	///
	/// let key = ThreadKey::get().unwrap();
	/// let p_err = mutex.lock(key).unwrap_err();
	/// let data: &PoisonGuard<_, _> = p_err.get_ref();
	/// println!("recovered {} items", data.len());
	/// ```
	#[must_use]
	pub const fn get_ref(&self) -> &Guard {
		&self.guard
	}

	/// Reaches into this error indicating that a lock is poisoned, returning a
	/// mutable reference to the underlying guard to allow access regardless.
	///
	/// # Examples
	///
	/// ```
	/// use std::collections::HashSet;
	/// use std::sync::Arc;
	/// use std::thread;
	///
	/// use happylock::{Mutex, Poisonable, ThreadKey};
	///
	/// let mutex = Arc::new(Poisonable::new(Mutex::new(HashSet::new())));
	///
	/// // poison the mutex
	/// let c_mutex = Arc::clone(&mutex);
	/// let _ = thread::spawn(move || {
	///     let key = ThreadKey::get().unwrap();
	///     let mut data = c_mutex.lock(key).unwrap();
	///     data.insert(10);
	///     panic!();
	/// }).join();
	///
	/// let key = ThreadKey::get().unwrap();
	/// let mut p_err = mutex.lock(key).unwrap_err();
	/// let data = p_err.get_mut();
	/// data.insert(20);
	/// println!("recovered {} items", data.len());
	/// ```
	#[must_use]
	pub fn get_mut(&mut self) -> &mut Guard {
		&mut self.guard
	}
}

impl<G, Key> fmt::Debug for TryLockPoisonableError<'_, '_, G, Key> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match *self {
			Self::Poisoned(..) => "Poisoned(..)".fmt(f),
			Self::WouldBlock(_) => "WouldBlock".fmt(f),
		}
	}
}

impl<G, Key> fmt::Display for TryLockPoisonableError<'_, '_, G, Key> {
	#[cfg_attr(test, mutants::skip)]
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match *self {
			Self::Poisoned(..) => "poisoned lock: another task failed inside",
			Self::WouldBlock(_) => "try_lock failed because the operation would block",
		}
		.fmt(f)
	}
}

impl<G, Key> Error for TryLockPoisonableError<'_, '_, G, Key> {}

impl<'flag, 'key, G, Key> From<PoisonError<PoisonGuard<'flag, 'key, G, Key>>>
	for TryLockPoisonableError<'flag, 'key, G, Key>
{
	fn from(value: PoisonError<PoisonGuard<'flag, 'key, G, Key>>) -> Self {
		Self::Poisoned(value)
	}
}
