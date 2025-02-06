use std::sync::Arc;

use happylock::collection::{BoxedLockCollection, RetryingLockCollection};
use happylock::mutex::Mutex;
use happylock::ThreadKey;
use lock_api::{GuardNoSend, RawMutex};

struct EvilMutex {
	inner: parking_lot::RawMutex,
}

unsafe impl RawMutex for EvilMutex {
	#[allow(clippy::declare_interior_mutable_const)]
	const INIT: Self = Self {
		inner: parking_lot::RawMutex::INIT,
	};

	type GuardMarker = GuardNoSend;

	fn lock(&self) {
		panic!("mwahahahaha");
	}

	fn try_lock(&self) -> bool {
		self.inner.try_lock()
	}

	unsafe fn unlock(&self) {
		panic!("mwahahahaha");
	}
}

#[test]
fn boxed_mutexes() {
	let mut key = ThreadKey::get().unwrap();
	let good_mutex: Arc<Mutex<i32, parking_lot::RawMutex>> = Arc::new(Mutex::new(5));
	let evil_mutex: Arc<Mutex<i32, EvilMutex>> = Arc::new(Mutex::new(7));
	let useless_mutex: Arc<Mutex<i32, parking_lot::RawMutex>> = Arc::new(Mutex::new(10));
	let c_good = Arc::clone(&good_mutex);
	let c_evil = Arc::clone(&evil_mutex);
	let c_useless = Arc::clone(&useless_mutex);

	let r = std::thread::spawn(move || {
		let mut key = ThreadKey::get().unwrap();
		let collection = BoxedLockCollection::try_new((&*c_good, &*c_evil, &*c_useless)).unwrap();
		collection.lock(&mut key);
	})
	.join();

	assert!(r.is_err());
	assert!(good_mutex.try_lock(&mut key).is_ok());
	assert!(evil_mutex.try_lock(&mut key).is_err());
	assert!(useless_mutex.try_lock(&mut key).is_ok());
}

#[test]
fn retrying_mutexes() {
	let mut key = ThreadKey::get().unwrap();
	let good_mutex: Arc<Mutex<i32, parking_lot::RawMutex>> = Arc::new(Mutex::new(5));
	let evil_mutex: Arc<Mutex<i32, EvilMutex>> = Arc::new(Mutex::new(7));
	let useless_mutex: Arc<Mutex<i32, parking_lot::RawMutex>> = Arc::new(Mutex::new(10));
	let c_good = Arc::clone(&good_mutex);
	let c_evil = Arc::clone(&evil_mutex);
	let c_useless = Arc::clone(&useless_mutex);

	let r = std::thread::spawn(move || {
		let mut key = ThreadKey::get().unwrap();
		let collection =
			RetryingLockCollection::try_new((&*c_good, &*c_evil, &*c_useless)).unwrap();
		collection.lock(&mut key);
	})
	.join();

	assert!(r.is_err());
	assert!(good_mutex.try_lock(&mut key).is_ok());
	assert!(evil_mutex.try_lock(&mut key).is_err());
	assert!(useless_mutex.try_lock(&mut key).is_ok());
}
