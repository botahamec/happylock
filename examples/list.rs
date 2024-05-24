use std::thread;

use happylock::{collection::RefLockCollection, Mutex, ThreadKey};

const N: usize = 10;

static DATA: [Mutex<usize>; 6] = [
	Mutex::new(0),
	Mutex::new(1),
	Mutex::new(2),
	Mutex::new(3),
	Mutex::new(4),
	Mutex::new(5),
];

static SEED: Mutex<u32> = Mutex::new(42);

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
	let mut threads = Vec::new();
	for _ in 0..N {
		let th = thread::spawn(move || {
			let mut key = ThreadKey::get().unwrap();
			loop {
				let mut data = Vec::new();
				for _ in 0..3 {
					let rand = random(&mut key);
					data.push(&DATA[rand % 6]);
				}

				let Some(lock) = RefLockCollection::try_new(&data) else {
					continue;
				};
				let mut guard = lock.lock(&mut key);
				*guard[0] += *guard[1];
				*guard[1] += *guard[2];
				*guard[2] += *guard[0];

				return;
			}
		});
		threads.push(th);
	}

	for th in threads {
		_ = th.join();
	}

	let key = ThreadKey::get().unwrap();
	let data = RefLockCollection::new(&DATA);
	let data = data.lock(key);
	for val in &*data {
		println!("{val}");
	}
}
