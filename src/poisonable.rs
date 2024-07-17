#[cfg(not(panic = "unwind"))]
use std::convert::Infallible;
use std::marker::PhantomData;
use std::sync::atomic::AtomicBool;

use crate::lockable::{Lockable, RawLock};

mod error;
mod flag;
mod guard;
mod poisonable;

#[derive(Debug, Default)]
struct PoisonFlag(#[cfg(panic = "unwind")] AtomicBool);

#[derive(Debug, Default)]
pub struct Poisonable<L: Lockable + RawLock> {
	inner: L,
	poisoned: PoisonFlag,
}

pub struct PoisonRef<'flag, G> {
	guard: G,
	#[cfg(panic = "unwind")]
	flag: &'flag PoisonFlag,
}

pub struct PoisonGuard<'flag, 'key, G, Key> {
	guard: PoisonRef<'flag, G>,
	key: Key,
	_phantom: PhantomData<&'key ()>,
}

pub struct PoisonError<Guard> {
	guard: Guard,
}

pub enum TryLockPoisonableError<'flag, 'key, G, Key: 'key> {
	Poisoned(PoisonError<PoisonGuard<'flag, 'key, G, Key>>),
	WouldBlock(Key),
}

pub type PoisonResult<Guard> = Result<Guard, PoisonError<Guard>>;

pub type TryLockPoisonableResult<'flag, 'key, G, Key> =
	Result<PoisonGuard<'flag, 'key, G, Key>, TryLockPoisonableError<'flag, 'key, G, Key>>;
