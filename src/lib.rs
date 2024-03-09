#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::declare_interior_mutable_const)]
#![allow(clippy::semicolon_if_nothing_returned)]

mod guard;
mod key;
mod lockable;

pub mod mutex;
pub mod rwlock;

pub use guard::LockGuard;
pub use key::{Key, ThreadKey};
pub use lockable::Lockable;
pub use mutex::SpinLock;

/// A mutual exclusion primitive useful for protecting shared data, which cannot deadlock.
///
/// By default, this uses `parking_lot` as a backend.
pub type Mutex<T> = mutex::Mutex<T, parking_lot::RawMutex>;

/// A reader-writer lock
///
/// By default, this uses `parking_lot` as a backend.
pub type RwLock<T> = rwlock::RwLock<T, parking_lot::RawRwLock>;
