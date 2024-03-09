#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::declare_interior_mutable_const)]
#![allow(clippy::semicolon_if_nothing_returned)]

mod guard;
mod key;
mod lockable;
pub mod mutex;

pub use guard::LockGuard;
pub use key::{Key, ThreadKey};
pub use lockable::Lockable;
pub use mutex::ParkingMutex as Mutex;
pub use mutex::SpinLock;
