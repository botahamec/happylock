use std::time::Duration;

use happylock::{collection::RetryingLockCollection, RwLock, ThreadKey};

static RWLOCK_1: RwLock<i32> = RwLock::new(1);
static RWLOCK_2: RwLock<i32> = RwLock::new(2);
static RWLOCK_3: RwLock<i32> = RwLock::new(3);

fn thread_1() {
	let key = ThreadKey::get().unwrap();
	let mut guard = RWLOCK_2.write(key);
	std::thread::sleep(Duration::from_millis(75));
	assert_eq!(*guard, 2);
	*guard = 5;
}

fn thread_2() {
	let key = ThreadKey::get().unwrap();
	let collection = RetryingLockCollection::try_new([&RWLOCK_1, &RWLOCK_2, &RWLOCK_3]).unwrap();
	std::thread::sleep(Duration::from_millis(25));
	let guard = collection.read(key);
	assert_eq!(*guard[0], 1);
	assert_eq!(*guard[1], 5);
	assert_eq!(*guard[2], 3);
}

#[test]
fn retries() {
	let t1 = std::thread::spawn(thread_1);
	let t2 = std::thread::spawn(thread_2);

	t1.join().unwrap();
	t2.join().unwrap();
}
