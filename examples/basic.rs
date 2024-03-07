use std::thread;

use happylock::mutex::Mutex;
use happylock::ThreadKey;

const N: usize = 10;

static DATA: Mutex<i32> = Mutex::new(0);

fn main() {
	for _ in 0..N {
		thread::spawn(move || {
			let key = ThreadKey::lock().unwrap();
			let mut data = DATA.lock(key);
			*data += 1;
		});
	}

	let key = ThreadKey::lock().unwrap();
	let data = DATA.lock(key);
	println!("{}", *data);
}
