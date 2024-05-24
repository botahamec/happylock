use std::thread;

use happylock::{Mutex, ThreadKey};

const N: usize = 10;

static DATA: Mutex<i32> = Mutex::new(0);

fn main() {
	let mut threads = Vec::new();
	for _ in 0..N {
		let th = thread::spawn(move || {
			let key = ThreadKey::get().unwrap();
			let mut data = DATA.lock(key);
			*data += 1;
		});
		threads.push(th);
	}

	for th in threads {
		_ = th.join();
	}

	let key = ThreadKey::get().unwrap();
	let data = DATA.lock(key);
	println!("{data}");
}
