use happylock::{Mutex, ThreadKey};

#[test]
fn no_new_threadkey_when_forgetting_lock() {
	let key = ThreadKey::get().unwrap();
	let mutex = Mutex::new("foo".to_string());

	let guard = mutex.lock(key);
	std::mem::forget(guard);

	assert!(ThreadKey::get().is_none());
}

#[test]
fn no_new_threadkey_in_scoped_lock() {
	let mut key = ThreadKey::get().unwrap();
	let mutex = Mutex::new("foo".to_string());

	mutex.scoped_lock(&mut key, |_| {
		assert!(ThreadKey::get().is_none());
	});

	mutex.lock(key);
}
