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
let data: Mutex<i32> = Mutex::new(0);

for _ in 0..N {
	thread::spawn(move || {
		// each thread gets one thread key
		let key = ThreadKey::get().unwrap();

		// unlocking a mutex requires a ThreadKey
		let mut data = data.lock(key);
		*data += 1;

		// the key is unlocked at the end of the scope
	});
}

let key = ThreadKey::get().unwrap();
let data = data.lock(&mut key);
println!("{}", *data);
```

Unlocking a mutex requires a `ThreadKey` or a mutable reference to `ThreadKey`. Each thread will be allowed to have one key at a time, but no more than that. The `ThreadKey` type is not cloneable or copyable. This means that only one thing can be locked at a time.

To lock multiple mutexes at a time, create a `LockCollection`.

```rust
static DATA_1: Mutex<i32> = Mutex::new(0);
static DATA_2: Mutex<String> = Mutex::new(String::new());

for _ in 0..N {
	thread::spawn(move || {
		let key = ThreadKey::get().unwrap();

		// happylock ensures at runtime there are no duplicate locks
		let collection = LockCollection::try_new((&DATA_1, &DATA_2)).unwrap();
		let mut guard = collection.lock(key);

		*guard.1 = (100 - *guard.0).to_string();
		*guard.0 += 1;
	});
}

let key = ThreadKey::get().unwrap();
let data = (&DATA_1, &DATA_2);
let data = LockGuard::lock(&data, &mut key);
println!("{}", *data.0);
println!("{}", *data.1);
```

## Performance

**The `ThreadKey` is a mostly-zero cost abstraction.** It doesn't use any memory, and it doesn't really exist at run-time. The only cost comes from calling `ThreadKey::get()`, because the function has to ensure at runtime that the key hasn't already been taken. Dropping the key will also have a small cost.

**Avoid `LockCollection::try_new`.** This constructor will check to make sure that the collection contains no duplicate locks. This is an O(n^2) operation, where n is the number of locks in the collection. `LockCollection::new` and `LockCollection::new_ref` don't need these checks because they use `OwnedLockable`, which is guaranteed to be unique as long as it is accessible. As a last resort, `LockCollection::new_unchecked` doesn't do this check, but is unsafe to call.

**Avoid using distinct lock orders for `LockCollection`.** The problem is that this library must iterate through the list of locks, and not complete until every single one of them is unlocked. This also means that attempting to lock multiple mutexes gives you a lower chance of ever running. Only one needs to be locked for the operation to need a reset. This problem can be prevented by not doing that in your code. Resources should be obtained in the same order on every thread.

## Future Work

Although this library is able to successfully prevent deadlocks, livelocks may still be an issue. Imagine thread 1 gets resource 1, thread 2 gets resource 2, thread 1 realizes it can't get resource 2, thread 2 realizes it can't get resource 1, thread 1 drops resource 1, thread 2 drops resource 2, and then repeat forever. In practice, this situation probably wouldn't last forever. But it would be nice if this could be prevented somehow. A more fair system for getting sets of locks would help, but I have no clue what that looks like.

It might to possible to break the `ThreadKey` system by having two crates import this crate and call `ThreadKey::get`. I'm not quite sure how this works, but Rust could decide to give each crate their own key, ergo one thread would get two keys. I don't think the standard library would have this issue. At a certain point, I have to recognize that someone could also just import the standard library mutex and get a deadlock that way.

We should add `Condvar` at some point. I didn't because I've never used it before, and I'm probably not the right person to solve this problem. I think all the synchronization problems could be solved by having `Condvar::wait` take a `ThreadKey` instead of a `MutexGuard`. Something similar can probably be done for `Barrier`. But again, I'm no expert.

Do `OnceLock` or `LazyLock` ever deadlock? We might not need to add those here.

It'd be nice to be able to use the mutexes built into the operating system, saving on binary size. Using `std::sync::Mutex` sounds promising, but it doesn't implement `RawMutex`, and implementing that is very difficult, if not impossible.

Personally, I don't like mutex poisoning, but maybe it can be worked into the library if you're into that sort of thing.

Are the ergonomics here any good? This is completely uncharted territory. Maybe there are some useful helper methods we don't have here yet. Maybe `try_lock` should return a `Result`. Maybe `lock_api` or `spin` implements some useful methods that I kept out for this proof of concept. Maybe there are some lock-specific methods that could be added to `LockCollection`. More types might be lockable using a `LockGuard`.

I want to try to get this working without the standard library. There are a few problems with this though. For instance, this crate uses `thread_local` to allow other threads to have their own keys. Also, the only practical type of mutex that would work is a spinlock. Although, more could be implemented using the `RawMutex` trait. The `LockCollection` requires memory allocation at this time in order to check for duplicate locks.

It'd be interesting to add some methods such as `lock_clone` or `lock_swap`. This would still require a thread key, in case the mutex is already locked. The only way this could be done without a thread key is with a `&mut Mutex<T>`, but we already have `get_mut`. A `try_lock_clone` or `try_lock_swap` might not need a `ThreadKey` though. A special lock that looks like `Cell` but implements `Sync` could be shared without a thread key, because the lock would be dropped immediately (preventing non-preemptive allocation). It might make some common operations easier.

There might be some use in trying to prevent circular wait. There could be a special type that only allows the locking mutexes in a specific order. This would still require a thread key so that nobody tries to unlock multiple lock sequences at the same time. The biggest problem is that `LockSequence::lock_next` would need to return the same value each time, which is not very flexible. Most use cases for this are solved already by using `LockCollection<OwnedLockable>`.

Some sort of `DynamicLock` type might be useful so that, for example, a `Mutex<usize>` and an `RwLock<usize>` could be unlocked at the same time inside of a `Vec<DynamicLock<usize>>`. Although, this wouldn't solve the problem of needing a `Mutex<usize>` and a `Mutex<String>` at the same time. This would be better solved usin the existing tuple system.
