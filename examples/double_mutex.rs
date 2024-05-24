use std::thread;

use happylock::{collection::RefLockCollection, Mutex, ThreadKey};

const N: usize = 10;

static DATA: (Mutex<i32>, Mutex<String>) = (Mutex::new(0), Mutex::new(String::new()));

fn main() {
	let mut threads = Vec::new();
	for _ in 0..N {
		let th = thread::spawn(move || {
			let key = ThreadKey::get().unwrap();
			let lock = RefLockCollection::new(&DATA);
			let mut guard = lock.lock(key);
			*guard.1 = (100 - *guard.0).to_string();
			*guard.0 += 1;
		});
		threads.push(th);
	}

	for th in threads {
		_ = th.join();
	}

	let key = ThreadKey::get().unwrap();
	let data = RefLockCollection::new(&DATA);
	let data = data.lock(key);
	println!("{}", data.0);
	println!("{}", data.1);
}
