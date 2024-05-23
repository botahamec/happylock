use std::{thread, time::Duration};

use happylock::{collection, Mutex, ThreadKey};

static PHILOSOPHERS: [Philosopher; 5] = [
	Philosopher {
		name: "Socrates",
		left: 0,
		right: 1,
	},
	Philosopher {
		name: "John Rawls",
		left: 1,
		right: 2,
	},
	Philosopher {
		name: "Jeremy Bentham",
		left: 2,
		right: 3,
	},
	Philosopher {
		name: "John Stuart Mill",
		left: 3,
		right: 4,
	},
	Philosopher {
		name: "Judith Butler",
		left: 4,
		right: 0,
	},
];

static FORKS: [Mutex<()>; 5] = [
	Mutex::new(()),
	Mutex::new(()),
	Mutex::new(()),
	Mutex::new(()),
	Mutex::new(()),
];

struct Philosopher {
	name: &'static str,
	left: usize,
	right: usize,
}

impl Philosopher {
	fn cycle(&self) {
		let key = ThreadKey::get().unwrap();
		thread::sleep(Duration::from_secs(1));

		// safety: no philosopher asks for the same fork twice
		let forks = [&FORKS[self.left], &FORKS[self.right]];
		let forks = unsafe { collection::RetryingLockCollection::new_unchecked(&forks) };
		let forks = forks.lock(key);
		println!("{} is eating...", self.name);
		thread::sleep(Duration::from_secs(1));
		println!("{} is done eating", self.name);
		drop(forks);
	}
}

fn main() {
	let handles: Vec<_> = PHILOSOPHERS
		.iter()
		.map(|philosopher| thread::spawn(move || philosopher.cycle()))
		// The `collect` is absolutely necessary, because we're using lazy
		// iterators. If `collect` isn't used, then the thread won't spawn
		// until we try to join on it.
		.collect();

	for handle in handles {
		_ = handle.join();
	}
}
