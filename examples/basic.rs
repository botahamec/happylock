use std::thread;

use happylock::mutex::{Mutex, SpinLock};
use happylock::ThreadKey;

const N: usize = 10;

static DATA: SpinLock<i32> = Mutex::new(0);

fn main() {
	for _ in 0..N {
		thread::spawn(move || {
			let mut key = ThreadKey::lock().unwrap();
			let mut data = DATA.lock(&mut key);
			*data += 1;
		});
	}

	let mut key = ThreadKey::lock().unwrap();
	let data = DATA.lock(&mut key);
	println!("{}", *data);
}
