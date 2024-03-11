#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::declare_interior_mutable_const)]
#![allow(clippy::semicolon_if_nothing_returned)]
#![allow(clippy::module_inception)]

//! As it turns out, the Rust borrow checker is powerful enough that, if the
//! standard library supported it, we could've made deadlocks undefined
//! behavior. This library currently serves as a proof of concept for how that
//! would work.
//!
//! # Theory
//!
//! There are four conditions necessary for a deadlock to occur. In order to
//! prevent deadlocks, we need to prevent one of the following:
//!
//! 1. mutual exclusion
//! 2. non-preemptive allocation
//! 3. circular wait
//! 4. **partial allocation**
//!
//! This library seeks to solve **partial allocation** by requiring total
//! allocation. All of the resources a thread needs must be allocated at the
//! same time. In order to request new resources, the old resources must be
//! dropped first. Requesting multiple resources at once is atomic. You either
//! get all of the requested resources or none at all.
//!
//! # Performance
//!
//! **Avoid [`LockCollection::try_new`].** This constructor will check to make
//! sure that the collection contains no duplicate locks. This is an O(n^2)
//! operation, where n is the number of locks in the collection.
//! [`LockCollection::new`] and [`LockCollection::new_ref`] don't need these
//! checks because they use [`OwnedLockable`], which is guaranteed to be unique
//! as long as it is accessible. As a last resort,
//! [`LockCollection::new_unchecked`] doesn't do this check, but is unsafe to
//! call.
//!
//! **Avoid using distinct lock orders for [`LockCollection`].** The problem is
//! that this library must iterate through the list of locks, and not complete
//! until every single one of them is unlocked. This also means that attempting
//! to lock multiple mutexes gives you a lower chance of ever running. Only one
//! needs to be locked for the operation to need a reset. This problem can be
//! prevented by not doing that in your code. Resources should be obtained in
//! the same order on every thread.
//!
//! # Examples
//!
//! Simple example:
//! ```
//! use std::thread;
//! use happylock::{Mutex, ThreadKey};
//!
//! const N: usize = 10;
//!
//! static DATA: Mutex<i32> = Mutex::new(0);
//!
//! for _ in 0..N {
//!     thread::spawn(move || {
//!         // each thread gets one thread key
//!         let key = ThreadKey::get().unwrap();
//!
//!         // unlocking a mutex requires a ThreadKey
//!         let mut data = DATA.lock(key);
//!         *data += 1;
//!
//!         // the key is unlocked at the end of the scope
//!     });
//! }
//!
//! let key = ThreadKey::get().unwrap();
//! let data = DATA.lock(key);
//! println!("{}", *data);
//! ```
//!
//! To lock multiple mutexes at a time, create a [`LockCollection`]:
//!
//! ```
//! use std::thread;
//! use happylock::{LockCollection, Mutex, ThreadKey};
//!
//! const N: usize = 10;
//!
//! static DATA_1: Mutex<i32> = Mutex::new(0);
//! static DATA_2: Mutex<String> = Mutex::new(String::new());
//!
//! for _ in 0..N {
//!     thread::spawn(move || {
//!         let key = ThreadKey::get().unwrap();
//!
//!         // happylock ensures at runtime there are no duplicate locks
//!         let collection = LockCollection::try_new((&DATA_1, &DATA_2)).unwrap();
//!         let mut guard = collection.lock(key);
//!
//!         *guard.1 = (100 - *guard.0).to_string();
//!         *guard.0 += 1;
//!     });
//! }
//!
//! let key = ThreadKey::get().unwrap();
//! let data = LockCollection::try_new((&DATA_1, &DATA_2)).unwrap();
//! let data = data.lock(key);
//! println!("{}", *data.0);
//! println!("{}", *data.1);
//! ```

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
