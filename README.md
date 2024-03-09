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
let data: SpinLock<i32> = Mutex::new(0);

for _ in 0..N {
	thread::spawn(move || {
		// each thread gets one thread key
		let key = ThreadKey::lock().unwrap();

		// unlocking a mutex requires a ThreadKey
		let mut data = data.lock(&mut key);
		*data += 1;

		// the key is unlocked at the end of the scope
	});
}

let key = ThreadKey::lock().unwrap();
let data = data.lock(&mut key);
println!("{}", *data);
```

Unlocking a mutex requires a `ThreadKey` or a mutable reference to `ThreadKey`. Each thread will be allowed to have one key at a time, but no more than that. The `ThreadKey` type is not cloneable or copyable. This means that only one thing can be locked at a time.

To lock multiple mutexes at a time, create a `LockGuard`.

```rust
static DATA_1: SpinLock<i32> = Mutex::new(0);
static DATA_2: SpinLock<String> = Mutex::new(String::new());

for _ in 0..N {
	thread::spawn(move || {
		let key = ThreadKey::lock().unwrap();
		let data = (&DATA_1, &DATA_2);
		let mut guard = LockGuard::lock(&data, &mut key);
		*guard.1 = (100 - *guard.0).to_string();
		*guard.0 += 1;
	});
}

let key = ThreadKey::lock().unwrap();
let data = (&DATA_1, &DATA_2);
let data = LockGuard::lock(&data, &mut key);
println!("{}", *data.0);
println!("{}", *data.1);
```

## Performance

The `ThreadKey` is a mostly-zero cost abstraction. It doesn't use any memory, and it doesn't really exist at run-time. The only cost comes from calling `ThreadKey::lock()`, because the function has to ensure at runtime that the key hasn't already been taken. Dropping the key will also have a small cost.

The real performance cost comes from the fact that the sets of multiple locks must be atomic. The problem is that this library must iterate through the list of locks, and not complete until every single one of them is unlocked. This also means that attempting to lock multiple mutexes gives you a lower chance of ever running. Only one needs to be locked for the operation to need a reset. This problem can be prevented by not doing that in your code. Resources should be obtained in the same order on every thread.

## Future Work

Are the ergonomics here any good? This is completely untreaded territory. Maybe there are some useful helper methods we don't have here yet. Maybe `try_lock` should return a `Result`. Maybe `lock_api` or `spin` implements some useful methods that I kept out for this proof of concept.

There might be some promise in trying to prevent circular wait. There could be a special type that only allows the locking mutexes in a specific order. This would still require a thread key so that nobody tries to unlock multiple lock sequences at the same time. But this could improve performance, since we wouldn't need to worry about making sure the lock operations are atomic.

Although this library is able to successfully prevent deadlocks, livelocks may still be an issue. Imagine thread 1 gets resource 1, thread 2 gets resource 2, thread 1 realizes it can't get resource 2, thread 2 realizes it can't get resource 1, thread 1 drops resource 1, thread 2 drops resource 2, and then repeat forever. In practice, this situation probably wouldn't last forever. But it would be nice if this could be prevented somehow.

I want to try to get this working without the standard library. There are a few problems with this though. For instance, this crate uses `thread_local` to allow other threads to have their own keys. Also, the only practical type of mutex that would work is a spinlock. Although, more could be implemented using the `RawMutex` trait.

Theoretically, it's possible to include the same mutex in a list twice, preventing the entire lock from being obtained. And this is technically a deadlock. A pretty easy to prevent deadlock, but a deadlock nonetheless. This is difficult to prevent, but could maybe be done by giving each mutex an ID, and then ensuring that the same ID doesn't appear twice in a list. This is an O(n^2) operation.

It'd be nice to be able to use the mutexes built into the operating system. Using `std::sync::Mutex` sounds promising, but it doesn't implement `RawMutex`, and implementing that is very difficult, if not impossible.

A more fair system for getting sets of locks would help, but I have no clue what that looks like.

A read-write lock would be very useful here, and maybe other primitives such as condvars and barriers?

Personally, I don't like mutex poisoning, but maybe it can be worked into the library if you're into that sort of thing.

More types might be lockable using a `LockGuard`.