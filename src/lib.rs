use parking_lot::Mutex;

thread_local! {
	// safety: this is the only place where a ThreadLock is created
	pub static KEY: Mutex<Option<ThreadKey>> = Mutex::new(Some(unsafe { ThreadKey::new() }));
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ThreadKey {
	_priv: *const (), // this isn't Send or Sync
}

impl ThreadKey {
	unsafe fn new() -> Self {
		Self {
			_priv: std::ptr::null(),
		}
	}

	pub fn lock() -> Option<Self> {
		KEY.with(|thread_lock| thread_lock.lock().take())
	}

	pub fn unlock(lock: ThreadKey) {
		KEY.with(|thread_lock| {
			let mut thread_lock = thread_lock.lock();
			*thread_lock = Some(lock);
		})
	}
}
