#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::declare_interior_mutable_const)]
#![allow(clippy::semicolon_if_nothing_returned)]

mod collection;
mod lockable;

pub mod key;
pub mod mutex;
pub mod rwlock;

pub use collection::LockCollection;
pub use lockable::Lockable;
pub use mutex::SpinLock;

/// The key for the current thread.
///
/// Only one of these exist per thread. To get the current thread's key, call
/// [`ThreadKey::lock`]. If the `ThreadKey` is dropped, it can be reobtained.
pub type ThreadKey = key::Key<'static>;

/// A mutual exclusion primitive useful for protecting shared data, which cannot deadlock.
///
/// By default, this uses `parking_lot` as a backend.
pub type Mutex<T> = mutex::Mutex<T, parking_lot::RawMutex>;

/// A reader-writer lock
///
/// By default, this uses `parking_lot` as a backend.
pub type RwLock<T> = rwlock::RwLock<T, parking_lot::RawRwLock>;
