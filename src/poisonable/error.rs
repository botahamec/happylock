use core::fmt;
use std::error::Error;

use super::{PoisonError, PoisonGuard, TryLockPoisonableError};

impl<Guard> fmt::Debug for PoisonError<Guard> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("PoisonError").finish_non_exhaustive()
	}
}

impl<Guard> fmt::Display for PoisonError<Guard> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		"poisoned lock: another task failed inside".fmt(f)
	}
}

impl<Guard> Error for PoisonError<Guard> {}

impl<'flag, 'key, G, Key> fmt::Debug for TryLockPoisonableError<'flag, 'key, G, Key> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match *self {
			Self::Poisoned(..) => "Poisoned(..)".fmt(f),
			Self::WouldBlock(_) => "WouldBlock".fmt(f),
		}
	}
}

impl<'flag, 'key, G, Key> fmt::Display for TryLockPoisonableError<'flag, 'key, G, Key> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match *self {
			Self::Poisoned(..) => "poisoned lock: another task failed inside",
			Self::WouldBlock(_) => "try_lock failed because the operation would block",
		}
		.fmt(f)
	}
}

impl<'flag, 'key, G, Key> Error for TryLockPoisonableError<'flag, 'key, G, Key> {}

impl<'flag, 'key, G, Key> From<PoisonError<PoisonGuard<'flag, 'key, G, Key>>>
	for TryLockPoisonableError<'flag, 'key, G, Key>
{
	fn from(value: PoisonError<PoisonGuard<'flag, 'key, G, Key>>) -> Self {
		Self::Poisoned(value)
	}
}
