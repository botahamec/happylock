use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use happylock::collection::{BoxedLockCollection, RetryingLockCollection};
use happylock::rwlock::RwLock;
use happylock::ThreadKey;
use lock_api::{GuardNoSend, RawRwLock};

struct EvilRwLock {
	inner: parking_lot::RawRwLock,
}

unsafe impl RawRwLock for EvilRwLock {
	#[allow(clippy::declare_interior_mutable_const)]
	const INIT: Self = Self {
		inner: parking_lot::RawRwLock::INIT,
	};

	type GuardMarker = GuardNoSend;

	fn lock_shared(&self) {
		panic!("mwahahahaha");
	}

	fn try_lock_shared(&self) -> bool {
		self.inner.try_lock_shared()
	}

	unsafe fn unlock_shared(&self) {
		panic!("mwahahahaha");
	}

	fn lock_exclusive(&self) {
		panic!("mwahahahaha");
	}

	fn try_lock_exclusive(&self) -> bool {
		self.inner.try_lock_exclusive()
	}

	unsafe fn unlock_exclusive(&self) {
		panic!("mwahahahaha");
	}
}

#[test]
fn boxed_rwlocks() {
	let mut key = ThreadKey::get().unwrap();
	let good_mutex: Arc<RwLock<i32, parking_lot::RawRwLock>> = Arc::new(RwLock::new(5));
	let evil_mutex: Arc<RwLock<i32, EvilRwLock>> = Arc::new(RwLock::new(7));
	let useless_mutex: Arc<RwLock<i32, parking_lot::RawRwLock>> = Arc::new(RwLock::new(10));
	let c_good = Arc::clone(&good_mutex);
	let c_evil = Arc::clone(&evil_mutex);
	let c_useless = Arc::clone(&useless_mutex);

	let r = std::thread::spawn(move || {
		let key = ThreadKey::get().unwrap();
		let collection = BoxedLockCollection::try_new((&*c_good, &*c_evil, &*c_useless)).unwrap();
		_ = collection.lock(key);
	})
	.join();

	assert!(r.is_err());
	assert!(good_mutex.scoped_try_write(&mut key, |_| {}).is_ok());
	assert!(evil_mutex.scoped_try_write(&mut key, |_| {}).is_err());
	assert!(useless_mutex.scoped_try_write(&mut key, |_| {}).is_ok());

	std::thread::scope(|s| {
		s.spawn(|| {
			let evil_mutex = AssertUnwindSafe(evil_mutex);
			let r = std::panic::catch_unwind(|| {
				let key = ThreadKey::get().unwrap();
				evil_mutex.write(key);
			});

			assert!(r.is_err());
		});

		s.spawn(|| {
			let key = ThreadKey::get().unwrap();
			good_mutex.write(key);
		});
	});
}

#[test]
fn retrying_rwlocks() {
	let mut key = ThreadKey::get().unwrap();
	let good_mutex: Arc<RwLock<i32, parking_lot::RawRwLock>> = Arc::new(RwLock::new(5));
	let evil_mutex: Arc<RwLock<i32, EvilRwLock>> = Arc::new(RwLock::new(7));
	let useless_mutex: Arc<RwLock<i32, parking_lot::RawRwLock>> = Arc::new(RwLock::new(10));
	let c_good = Arc::clone(&good_mutex);
	let c_evil = Arc::clone(&evil_mutex);
	let c_useless = Arc::clone(&useless_mutex);

	let r = std::thread::spawn(move || {
		let key = ThreadKey::get().unwrap();
		let collection =
			RetryingLockCollection::try_new((&*c_good, &*c_evil, &*c_useless)).unwrap();
		collection.lock(key);
	})
	.join();

	assert!(r.is_err());
	assert!(good_mutex.scoped_try_write(&mut key, |_| {}).is_ok());
	assert!(evil_mutex.scoped_try_write(&mut key, |_| {}).is_err());
	assert!(useless_mutex.scoped_try_write(&mut key, |_| {}).is_ok());
}
