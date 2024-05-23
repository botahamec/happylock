use happylock::{LockCollection, Mutex, ThreadKey};
use std::thread;

const N: usize = 36;

static DATA: [Mutex<i32>; 2] = [Mutex::new(0), Mutex::new(1)];

fn main() {
	for _ in 0..N {
		thread::spawn(move || {
			let key = ThreadKey::get().unwrap();

			// a reference to a type that implements `OwnedLockable` will never
			// contain duplicates, so no duplicate checking is needed.
			let collection = LockCollection::new_ref(&DATA);
			let mut guard = collection.lock(key);

			let x = *guard[1];
			*guard[1] += *guard[0];
			*guard[0] = x;
		});
	}

	let key = ThreadKey::get().unwrap();
	let data = LockCollection::new_ref(&DATA);
	let data = data.lock(key);
	println!("{}", data[0]);
	println!("{}", data[1]);
}
