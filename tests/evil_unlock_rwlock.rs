use std::sync::Arc;

use happylock::collection::{BoxedLockCollection, RetryingLockCollection};
use happylock::rwlock::RwLock;
use happylock::ThreadKey;
use lock_api::{GuardNoSend, RawRwLock};

struct KindaEvilRwLock {
	inner: parking_lot::RawRwLock,
}

struct EvilRwLock {}

unsafe impl RawRwLock for KindaEvilRwLock {
	#[allow(clippy::declare_interior_mutable_const)]
	const INIT: Self = Self {
		inner: parking_lot::RawRwLock::INIT,
	};

	type GuardMarker = GuardNoSend;

	fn lock_shared(&self) {
		self.inner.lock_shared()
	}

	fn try_lock_shared(&self) -> bool {
		self.inner.try_lock_shared()
	}

	unsafe fn unlock_shared(&self) {
		panic!("mwahahahaha");
	}

	fn lock_exclusive(&self) {
		self.inner.lock_exclusive()
	}

	fn try_lock_exclusive(&self) -> bool {
		self.inner.try_lock_exclusive()
	}

	unsafe fn unlock_exclusive(&self) {
		panic!("mwahahahaha");
	}
}

unsafe impl RawRwLock for EvilRwLock {
	#[allow(clippy::declare_interior_mutable_const)]
	const INIT: Self = Self {};

	type GuardMarker = GuardNoSend;

	fn lock_shared(&self) {
		panic!("mwahahahaha");
	}

	fn try_lock_shared(&self) -> bool {
		panic!("mwahahahaha");
	}

	unsafe fn unlock_shared(&self) {
		panic!("mwahahahaha");
	}

	fn lock_exclusive(&self) {
		panic!("mwahahahaha");
	}

	fn try_lock_exclusive(&self) -> bool {
		panic!("mwahahahaha")
	}

	unsafe fn unlock_exclusive(&self) {
		panic!("mwahahahaha");
	}
}

#[test]
fn boxed_rwlocks() {
	let mut key = ThreadKey::get().unwrap();
	let kinda_evil_mutex: RwLock<i32, KindaEvilRwLock> = RwLock::new(5);
	let evil_mutex: RwLock<i32, EvilRwLock> = RwLock::new(7);
	let useless_mutex: RwLock<i32, parking_lot::RawRwLock> = RwLock::new(10);

	let r = std::thread::scope(|s| {
		let r = s
			.spawn(|| {
				let key = ThreadKey::get().unwrap();
				let collection =
					BoxedLockCollection::try_new((&kinda_evil_mutex, &evil_mutex, &useless_mutex))
						.unwrap();
				_ = collection.read(key);
			})
			.join();

		r
	});

	assert!(r.is_err());
	assert!(kinda_evil_mutex.scoped_try_write(&mut key, |_| {}).is_err());
	assert!(evil_mutex.scoped_try_write(&mut key, |_| {}).is_err());
	assert!(useless_mutex.scoped_try_write(&mut key, |_| {}).is_ok());
}

#[test]
fn retrying_rwlocks() {
	let mut key = ThreadKey::get().unwrap();
	let kinda_evil_mutex: Arc<RwLock<i32, KindaEvilRwLock>> = Arc::new(RwLock::new(5));
	let evil_mutex: Arc<RwLock<i32, EvilRwLock>> = Arc::new(RwLock::new(7));
	let useless_mutex: Arc<RwLock<i32, parking_lot::RawRwLock>> = Arc::new(RwLock::new(10));
	let c_good = Arc::clone(&kinda_evil_mutex);
	let c_evil = Arc::clone(&evil_mutex);
	let c_useless = Arc::clone(&useless_mutex);

	let r = std::thread::spawn(move || {
		let key = ThreadKey::get().unwrap();
		let collection =
			RetryingLockCollection::try_new((&*c_good, &*c_evil, &*c_useless)).unwrap();
		collection.read(key);
	})
	.join();

	assert!(r.is_err());
	assert!(kinda_evil_mutex.scoped_try_write(&mut key, |_| {}).is_err());
	assert!(evil_mutex.scoped_try_write(&mut key, |_| {}).is_err());
	assert!(useless_mutex.scoped_try_write(&mut key, |_| {}).is_ok());
}
