#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::declare_interior_mutable_const)]
#![allow(clippy::semicolon_if_nothing_returned)]
#![allow(clippy::module_inception)]

mod collection;
mod key;
mod lockable;

pub mod mutex;
pub mod rwlock;

pub use collection::LockCollection;
pub use key::{Keyable, ThreadKey};
pub use lockable::{Lockable, OwnedLockable};

#[cfg(feature = "spin")]
pub use mutex::SpinLock;

/// A mutual exclusion primitive useful for protecting shared data, which cannot deadlock.
///
/// By default, this uses `parking_lot` as a backend.
#[cfg(feature = "parking_lot")]
pub type Mutex<T> = mutex::Mutex<T, parking_lot::RawMutex>;

/// A reader-writer lock
///
/// By default, this uses `parking_lot` as a backend.
#[cfg(feature = "parking_lot")]
pub type RwLock<T> = rwlock::RwLock<T, parking_lot::RawRwLock>;
