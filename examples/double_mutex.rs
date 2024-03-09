use std::thread;

use happylock::{LockGuard, Mutex, ThreadKey};

const N: usize = 10;

static DATA_1: Mutex<i32> = Mutex::new(0);
static DATA_2: Mutex<String> = Mutex::new(String::new());

fn main() {
	let mut threads = Vec::new();
	for _ in 0..N {
		let th = thread::spawn(move || {
			let key = ThreadKey::lock().unwrap();
			let data = (&DATA_1, &DATA_2);
			let mut guard = LockGuard::lock(&data, key);
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
	let data = LockGuard::lock(&data, key);
	println!("{}", *data.0);
	println!("{}", *data.1);
}
