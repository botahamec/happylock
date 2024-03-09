use std::thread;

use happylock::{LockCollection, Mutex, ThreadKey};

const N: usize = 10;

static DATA_1: Mutex<i32> = Mutex::new(0);
static DATA_2: Mutex<String> = Mutex::new(String::new());

fn main() {
	let mut threads = Vec::new();
	for _ in 0..N {
		let th = thread::spawn(move || {
			let key = ThreadKey::lock().unwrap();
			let data = (&DATA_1, &DATA_2);
			let lock = LockCollection::new(data).unwrap();
			let mut guard = lock.lock(key);
			*guard.1 = (100 - *guard.0).to_string();
			*guard.0 += 1;
		});
		threads.push(th);
	}

	for th in threads {
		_ = th.join();
	}

	let key = ThreadKey::lock().unwrap();
	let data = (&DATA_1, &DATA_2);
	let data = LockCollection::new(data).unwrap();
	let data = data.lock(key);
	println!("{}", *data.0);
	println!("{}", *data.1);
}
