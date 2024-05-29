#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::declare_interior_mutable_const)]
#![allow(clippy::semicolon_if_nothing_returned)]
#![allow(clippy::module_inception)]
#![allow(clippy::single_match_else)]

//! As it turns out, the Rust borrow checker is powerful enough that, if the
//! standard library supported it, we could've made deadlocks undefined
//! behavior. This library currently serves as a proof of concept for how that
//! would work.
//!
//! # Theory
//!
//! There are four conditions necessary for a deadlock to occur. In order to
//! prevent deadlocks, we just need to prevent one of the following:
//!
//! 1. mutual exclusion
//! 2. non-preemptive allocation
//! 3. circular wait
//! 4. **partial allocation**
//!
//! This library seeks to solve **partial allocation** by requiring total
//! allocation. All the resources a thread needs must be allocated at the same
//! time. In order to request new resources, the old resources must be dropped
//! first. Requesting multiple resources at once is atomic. You either get all
//! the requested resources or none at all.
//!
//! As an optimization, this library also often prevents **circular wait**.
//! Many collections sort the locks in order of their memory address. As long
//! as the locks are always acquired in that order, then time doesn't need to
//! be wasted on releasing locks after a failure and re-acquiring them later.
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
//!
//! In many cases, the [`LockCollection::new`] or [`LockCollection::new_ref`]
//! method can be used, improving performance.
//!
//! ```rust
//! use std::thread;
//! use happylock::{LockCollection, Mutex, ThreadKey};
//!
//! const N: usize = 32;
//!
//! static DATA: [Mutex<i32>; 2] = [Mutex::new(0), Mutex::new(1)];
//!
//! for _ in 0..N {
//!     thread::spawn(move || {
//!         let key = ThreadKey::get().unwrap();
//!
//!         // a reference to a type that implements `OwnedLockable` will never
//!         // contain duplicates, so no duplicate checking is needed.
//!         let collection = LockCollection::new_ref(&DATA);
//!         let mut guard = collection.lock(key);
//!
//!         let x = *guard[1];
//!         *guard[1] += *guard[0];
//!         *guard[0] = x;
//!     });
//! }
//!
//! let key = ThreadKey::get().unwrap();
//! let data = LockCollection::new_ref(&DATA);
//! let data = data.lock(key);
//! println!("{}", data[0]);
//! println!("{}", data[1]);
//! ```
//!
//! # Performance
//!
//! **The `ThreadKey` is a mostly-zero cost abstraction.** It doesn't use any
//! memory, and it doesn't really exist at run-time. The only cost comes from
//! calling `ThreadKey::get()`, because the function has to ensure at runtime
//! that the key hasn't already been taken. Dropping the key will also have a
//! small cost.
//!
//! **Consider [`OwnedLockCollection`].** This will almost always be the
//! fastest lock collection. It doesn't expose the underlying collection
//! immutably, which means that it will always be locked in the same order, and
//! doesn't need any sorting.
//!
//! **Avoid [`LockCollection::try_new`].** This constructor will check to make
//! sure that the collection contains no duplicate locks. In most cases, this
//! is O(nlogn), where n is the number of locks in the collections but in the
//! case of [`RetryingLockCollection`], it's close to O(n).
//! [`LockCollection::new`] and [`LockCollection::new_ref`] don't need these
//! checks because they use [`OwnedLockable`], which is guaranteed to be unique
//! as long as it is accessible. As a last resort,
//! [`LockCollection::new_unchecked`] doesn't do this check, but is unsafe to
//! call.
//!
//! **Know how to use [`RetryingLockCollection`].** This collection doesn't do
//! any sorting, but uses a wasteful lock algorithm. It can't rely on the order
//! of the locks to be the same across threads, so if it finds a lock that it
//! can't acquire without blocking, it'll first release all of the locks it
//! already acquired to avoid blocking other threads. This is wasteful because
//! this algorithm may end up re-acquiring the same lock multiple times. To
//! avoid this, ensure that (1) the first lock in the collection is always the
//! first lock in any collection it appears in, and (2) the other locks in the
//! collection are always preceded by that first lock. This will prevent any
//! wasted time from re-acquiring locks. If you're unsure, [`LockCollection`]
//! is a sensible default.
//!
//! [`OwnedLockable`]: `lockable::OwnedLockable`
//! [`OwnedLockCollection`]: `collection::OwnedLockCollection`
//! [`RetryingLockCollection`]: `collection::RetryingLockCollection`

mod key;

pub mod collection;
pub mod lockable;
pub mod mutex;
pub mod rwlock;

pub use key::{Keyable, ThreadKey};

#[cfg(feature = "spin")]
pub use mutex::SpinLock;

// Personally, I think re-exports look ugly in the rust documentation, so I
// went with type aliases instead.

/// A collection of locks that can be acquired simultaneously.
///
/// This re-exports [`BoxedLockCollection`] as a sensible default.
///
/// [`BoxedLockCollection`]: collection::BoxedLockCollection
pub type LockCollection<L> = collection::BoxedLockCollection<L>;

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
