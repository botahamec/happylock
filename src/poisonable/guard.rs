use std::fmt::{Debug, Display};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering::Relaxed;

use crate::Keyable;

use super::{PoisonGuard, PoisonRef};

impl<'flag, Guard> Drop for PoisonRef<'flag, Guard> {
	fn drop(&mut self) {
		#[cfg(panic = "unwind")]
		if std::thread::panicking() {
			self.flag.0.store(true, Relaxed);
		}
	}
}

impl<'flag, Guard: Debug> Debug for PoisonRef<'flag, Guard> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<'flag, Guard: Display> Display for PoisonRef<'flag, Guard> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}

impl<'flag, Guard> Deref for PoisonRef<'flag, Guard> {
	type Target = Guard;

	fn deref(&self) -> &Self::Target {
		&self.guard
	}
}

impl<'flag, Guard> DerefMut for PoisonRef<'flag, Guard> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.guard
	}
}

impl<'flag, Guard> AsRef<Guard> for PoisonRef<'flag, Guard> {
	fn as_ref(&self) -> &Guard {
		&self.guard
	}
}

impl<'flag, Guard> AsMut<Guard> for PoisonRef<'flag, Guard> {
	fn as_mut(&mut self) -> &mut Guard {
		&mut self.guard
	}
}

impl<'flag, 'key, Guard: Debug, Key: Keyable> Debug for PoisonGuard<'flag, 'key, Guard, Key> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<'flag, 'key, Guard: Display, Key: Keyable> Display for PoisonGuard<'flag, 'key, Guard, Key> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}

impl<'flag, 'key, Guard, Key: Keyable> Deref for PoisonGuard<'flag, 'key, Guard, Key> {
	type Target = Guard;

	fn deref(&self) -> &Self::Target {
		&self.guard.guard
	}
}

impl<'flag, 'key, Guard, Key: Keyable> DerefMut for PoisonGuard<'flag, 'key, Guard, Key> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.guard.guard
	}
}

impl<'flag, 'key, Guard, Key: Keyable> AsRef<Guard> for PoisonGuard<'flag, 'key, Guard, Key> {
	fn as_ref(&self) -> &Guard {
		&self.guard.guard
	}
}

impl<'flag, 'key, Guard, Key: Keyable> AsMut<Guard> for PoisonGuard<'flag, 'key, Guard, Key> {
	fn as_mut(&mut self) -> &mut Guard {
		&mut self.guard.guard
	}
}
