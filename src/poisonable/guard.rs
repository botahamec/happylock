use std::fmt::{Debug, Display};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering::Relaxed;

use crate::Keyable;

use super::{PoisonFlag, PoisonGuard, PoisonRef};

impl<'a, Guard> PoisonRef<'a, Guard> {
	// This is used so that we don't keep accidentally adding the flag reference
	pub(super) const fn new(flag: &'a PoisonFlag, guard: Guard) -> Self {
		Self {
			guard,
			#[cfg(panic = "unwind")]
			flag,
			_phantom: PhantomData,
		}
	}
}

impl<Guard> Drop for PoisonRef<'_, Guard> {
	fn drop(&mut self) {
		#[cfg(panic = "unwind")]
		if std::thread::panicking() {
			self.flag.0.store(true, Relaxed);
		}
	}
}

impl<Guard: Debug> Debug for PoisonRef<'_, Guard> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&**self, f)
	}
}

impl<Guard: Display> Display for PoisonRef<'_, Guard> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&**self, f)
	}
}

impl<Guard> Deref for PoisonRef<'_, Guard> {
	type Target = Guard;

	fn deref(&self) -> &Self::Target {
		&self.guard
	}
}

impl<Guard> DerefMut for PoisonRef<'_, Guard> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.guard
	}
}

impl<Guard> AsRef<Guard> for PoisonRef<'_, Guard> {
	fn as_ref(&self) -> &Guard {
		&self.guard
	}
}

impl<Guard> AsMut<Guard> for PoisonRef<'_, Guard> {
	fn as_mut(&mut self) -> &mut Guard {
		&mut self.guard
	}
}

impl<Guard: Debug, Key: Keyable> Debug for PoisonGuard<'_, '_, Guard, Key> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&self.guard, f)
	}
}

impl<Guard: Display, Key: Keyable> Display for PoisonGuard<'_, '_, Guard, Key> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&self.guard, f)
	}
}

impl<T, Guard: Deref<Target = T>, Key: Keyable> Deref for PoisonGuard<'_, '_, Guard, Key> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		#[allow(clippy::explicit_auto_deref)] // fixing this results in a compiler error
		&*self.guard.guard
	}
}

impl<T, Guard: DerefMut<Target = T>, Key: Keyable> DerefMut for PoisonGuard<'_, '_, Guard, Key> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		#[allow(clippy::explicit_auto_deref)] // fixing this results in a compiler error
		&mut *self.guard.guard
	}
}

impl<Guard, Key: Keyable> AsRef<Guard> for PoisonGuard<'_, '_, Guard, Key> {
	fn as_ref(&self) -> &Guard {
		&self.guard.guard
	}
}

impl<Guard, Key: Keyable> AsMut<Guard> for PoisonGuard<'_, '_, Guard, Key> {
	fn as_mut(&mut self) -> &mut Guard {
		&mut self.guard.guard
	}
}
