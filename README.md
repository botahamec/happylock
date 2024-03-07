# HappyLock: Deadlock Free Mutexes

As it turns out, the Rust borrow checker is powerful enough that, if the
standard library supported it, we could've made deadlocks undefined behavior.
This library currently serves as a proof of concept for how that would work.

## Theory

There are four conditions necessary for a deadlock to occur. In order to
prevent deadlocks, we need to prevent one of the following:

1. mutual exclusion (This is the entire point of a mutex, so we can't prevent that)
2. non-preemptive allocation (The language must be able to take away a mutex from a thread at any time. This would be very annoying.)
3. circular wait (The language must enforce that every thread locks mutexes in the exact same order)
4. partial allocation (The language must enforce total allocation)

This library prevents #4, by requiring that all of the resources that a thread needs be locked at once. This is an atomic operation, so either all locks will be acquired, or none will.

## Example

```rust
let data = Mutex::new(0);

for _ in 0..N {
	thread::spawn(move || {
		// each thread gets one thread key
		let key = ThreadKey::lock().unwrap();

		// unlocking a mutex requires a ThreadKey
		let mut data = data.lock(key);
		*data += 1;

		// the key is unlocked at the end of the scope
	});
}

let key = ThreadKey::lock().unwrap();
let data = data.lock(key);
println!("{}", *data);
```

Unlocking a mutex requires a `ThreadKey`. Each thread will be allowed to have one key at a time, but no more than that. The `ThreadKey` type is not cloneable or copyable. To get the key back out of a mutex, it must be unlocked. This means that only one thing can be locked at a time.

To lock multiple mutexes at a time, create a `LockGuard`.

```rust
static DATA_1: Mutex<i32> = Mutex::new(0);
static DATA_2: Mutex<String> = Mutex::new(String::new());

for _ in 0..N {
	thread::spawn(move || {
		let key = ThreadKey::lock().unwrap();
		let data = (&DATA_1, &DATA_2);
		let mut guard = LockGuard::lock(&data, key);
		*guard.1 = (100 - *guard.0).to_string();
		*guard.0 += 1;
	});
}

let key = ThreadKey::lock().unwrap();
let data = (&DATA_1, &DATA_2);
let data = LockGuard::lock(&data, key);
println!("{}", *data.0);
println!("{}", *data.1);
```

## Performance

The `ThreadKey` is a mostly-zero cost abstraction. It doesn't use any memory, and it doesn't really exist at run-time. The only cost comes from calling `ThreadKey::lock()`, because the function has to ensure that the key hasn't already been taken. Dropping the key will also have a small cost.

The real performance cost comes from the fact that the sets of multiple locks must be atomic. The problem is that this library must iterate through the list of locks, and not complete until every single one of them is unlocked. This also means that attempting to lock multiple mutexes gives you a lower chance of ever running. Only one needs to be locked for the operation to need a reset. This problem can be prevented by not doing that in your code. Resources should be obtained in the same order on every thread.

Currently, a spinlock is used as the mutex. This should be fixed in future releases.

## Future Work

Are the ergonomics really correct here? This is completely untreaded territory. Maybe there are some useful helper methods we don't have here yet.

There might be some promise in trying to prevent circular wait. There could be a special type that only allows the locking mutexes in a specific order. This would still require a thread key so that nobody tries to unlock multiple lock sequences at the same time. But this could improve performance, since we wouldn't need to worry about making sure the lock operations are atomic.

Currently, the mutex is implemented using a spinlock. We need to not do that. We could use parking lot, or mutexes from the operating system.

A more fair system for getting sets locks would help, but I have no clue what that will look like.

A read-write lock would be very useful here, and maybe condvars?

Personally, I don't like mutex poisoning, but maybe it can be worked into the library if you're into that sort of thing.

More types should be lockable using a `LockGuard`.