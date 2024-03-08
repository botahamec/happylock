#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::declare_interior_mutable_const)]

mod guard;
mod lock;
mod lockable;
pub mod mutex;

pub use guard::LockGuard;
pub use lock::{Key, ThreadKey};
pub use lockable::Lockable;
pub use mutex::Mutex;
