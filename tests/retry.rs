use std::time::Duration;

use happylock::{collection::RetryingLockCollection, Mutex, ThreadKey};

static MUTEX_1: Mutex<i32> = Mutex::new(1);
static MUTEX_2: Mutex<i32> = Mutex::new(2);
static MUTEX_3: Mutex<i32> = Mutex::new(3);

fn thread_1() {
	let key = ThreadKey::get().unwrap();
	let mut guard = MUTEX_2.lock(key);
	std::thread::sleep(Duration::from_millis(100));
	*guard = 5;
}

fn thread_2() {
	let mut key = ThreadKey::get().unwrap();
	std::thread::sleep(Duration::from_millis(50));
	let collection = RetryingLockCollection::try_new([&MUTEX_1, &MUTEX_2, &MUTEX_3]).unwrap();
	collection.scoped_lock(&mut key, |guard| {
		assert_eq!(*guard[0], 4);
		assert_eq!(*guard[1], 5);
		assert_eq!(*guard[2], 3);
	});
}

fn thread_3() {
	let key = ThreadKey::get().unwrap();
	std::thread::sleep(Duration::from_millis(75));
	let mut guard = MUTEX_1.lock(key);
	std::thread::sleep(Duration::from_millis(100));
	*guard = 4;
}

fn thread_4() {
	let mut key = ThreadKey::get().unwrap();
	std::thread::sleep(Duration::from_millis(25));
	let collection = RetryingLockCollection::try_new([&MUTEX_1, &MUTEX_2]).unwrap();
	assert!(collection.scoped_try_lock(&mut key, |_| {}).is_err());
}

#[test]
fn retries() {
	let t1 = std::thread::spawn(thread_1);
	let t2 = std::thread::spawn(thread_2);
	let t3 = std::thread::spawn(thread_3);
	let t4 = std::thread::spawn(thread_4);

	t1.join().unwrap();
	t2.join().unwrap();
	t3.join().unwrap();
	t4.join().unwrap();
}
