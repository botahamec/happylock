use std::thread;

use happylock::mutex::{Mutex, SpinLock};
use happylock::{LockGuard, ThreadKey};

const N: usize = 10;

static DATA: [SpinLock<usize>; 6] = [
	Mutex::new(0),
	Mutex::new(1),
	Mutex::new(2),
	Mutex::new(3),
	Mutex::new(4),
	Mutex::new(5),
];

static SEED: SpinLock<u32> = Mutex::new(42);

fn random(key: &mut ThreadKey) -> usize {
	let mut seed = SEED.lock(key);
	let x = *seed;
	let x = x ^ (x << 13);
	let x = x ^ (x >> 17);
	let x = x ^ (x << 5);
	*seed = x;
	x as usize
}

fn main() {
	for _ in 0..N {
		thread::spawn(move || {
			let mut key = ThreadKey::lock().unwrap();
			let mut data = Vec::new();
			for _ in 0..3 {
				let rand = random(&mut key);
				data.push(&DATA[rand % 6]);
			}

			let data = [data[0], data[1], data[2]];
			let mut guard = LockGuard::lock(&data, &mut key);
			*guard[0] += *guard[1];
			*guard[1] += *guard[2];
			*guard[2] += *guard[0];
		});
	}

	let mut key = ThreadKey::lock().unwrap();
	let data = LockGuard::lock(&DATA, &mut key);
	for val in &*data {
		println!("{}", **val);
	}
}
