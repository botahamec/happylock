use core::fmt;
use std::error::Error;

use super::{PoisonError, PoisonGuard, TryLockPoisonableError};

#[mutants::skip]
#[cfg(not(tarpaulin_include))]
impl<Guard> fmt::Debug for PoisonError<Guard> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("PoisonError").finish_non_exhaustive()
	}
}

impl<Guard> fmt::Display for PoisonError<Guard> {
	#[cfg_attr(test, mutants::skip)]
	#[cfg(not(tarpaulin_include))]
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
	/// use std::thread;
	///
	/// use happylock::{Mutex, Poisonable, ThreadKey};
	///
	/// let mutex = Poisonable::new(Mutex::new(HashSet::new()));
	///
	/// // poison the mutex
	/// thread::scope(|s| {
	///     let r = s.spawn(|| {
	///         let key = ThreadKey::get().unwrap();
	///         let mut data = mutex.lock(key).unwrap();
	///         data.insert(10);
	///         panic!();
	///     }).join();
	/// });
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
	/// use std::thread;
	///
	/// use happylock::{Mutex, Poisonable, ThreadKey};
	/// use happylock::poisonable::PoisonGuard;
	///
	/// let mutex = Poisonable::new(Mutex::new(HashSet::new()));
	///
	/// // poison the mutex
	/// thread::scope(|s| {
	///     let r = s.spawn(|| {
	///         let key = ThreadKey::get().unwrap();
	///         let mut data = mutex.lock(key).unwrap();
	///         data.insert(10);
	///         panic!();
	///     }).join();
	/// });
	///
	/// let key = ThreadKey::get().unwrap();
	/// let p_err = mutex.lock(key).unwrap_err();
	/// let data: &PoisonGuard<_> = p_err.get_ref();
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
	/// use std::thread;
	///
	/// use happylock::{Mutex, Poisonable, ThreadKey};
	///
	/// let mutex =Poisonable::new(Mutex::new(HashSet::new()));
	///
	/// // poison the mutex
	/// thread::scope(|s| {
	///     let r = s.spawn(|| {
	///         let key = ThreadKey::get().unwrap();
	///         let mut data = mutex.lock(key).unwrap();
	///         data.insert(10);
	///         panic!();
	///     }).join();
	/// });
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

#[mutants::skip]
#[cfg(not(tarpaulin_include))]
impl<G> fmt::Debug for TryLockPoisonableError<'_, G> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match *self {
			Self::Poisoned(..) => "Poisoned(..)".fmt(f),
			Self::WouldBlock(_) => "WouldBlock".fmt(f),
		}
	}
}

impl<G> fmt::Display for TryLockPoisonableError<'_, G> {
	#[cfg_attr(test, mutants::skip)]
	#[cfg(not(tarpaulin_include))]
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match *self {
			Self::Poisoned(..) => "poisoned lock: another task failed inside",
			Self::WouldBlock(_) => "try_lock failed because the operation would block",
		}
		.fmt(f)
	}
}

impl<G> Error for TryLockPoisonableError<'_, G> {}

impl<'flag, G> From<PoisonError<PoisonGuard<'flag, G>>> for TryLockPoisonableError<'flag, G> {
	fn from(value: PoisonError<PoisonGuard<'flag, G>>) -> Self {
		Self::Poisoned(value)
	}
}
