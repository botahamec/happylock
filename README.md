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

This library seeks to solve **partial allocation** by requiring total
allocation. All the resources a thread needs must be allocated at the same
time. In order to request new resources, the old resources must be dropped
first. Requesting multiple resources at once is atomic. You either get all the
requested resources or none at all.

As an optimization, this library also often prevents **circular wait**. Many
collections sort the locks in order of their memory address. As long as the
locks are always acquired in that order, then time doesn't need to be wasted
on releasing locks after a failure and re-acquiring them later.

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

Unlocking a mutex requires a `ThreadKey` or a mutable reference to `ThreadKey`.
Each thread will be allowed to have one key at a time, but no more than that.
The `ThreadKey` type is not cloneable or copyable. This means that only one
thing can be locked at a time.

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

In many cases, the [`LockCollection::new`] or [`LockCollection::new_ref`]
method can be used, improving performance.

```rust
use std::thread;
use happylock::{LockCollection, Mutex, ThreadKey};

const N: usize = 100;

static DATA: [Mutex<i32>; 2] = [Mutex::new(0), Mutex::new(1)];

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
```

## Performance

**The `ThreadKey` is a mostly-zero cost abstraction.** It doesn't use any memory, and it doesn't really exist at run-time. The only cost comes from calling `ThreadKey::get()`, because the function has to ensure at runtime that the key hasn't already been taken. Dropping the key will also have a small cost.

**Consider [`OwnedLockCollection`].** This will almost always be the fastest lock collection. It doesn't expose the underlying collection immutably, which means that it will always be locked in the same order, and doesn't need any sorting.

**Avoid [`LockCollection::try_new`].** This constructor will check to make sure that the collection contains no duplicate locks. In most cases, this is O(nlogn), where n is the number of locks in the collections but in the case of [`RetryingLockCollection`], it's close to O(n). [`LockCollection::new`] and [`LockCollection::new_ref`] don't need these checks because they use [`OwnedLockable`], which is guaranteed to be unique as long as it is accessible. As a last resort [`LockCollection::new_unchecked`] doesn't do this check, but is unsafe to call.

**Know how to use [`RetryingLockCollection`].** This collection doesn't do any sorting, but uses a wasteful lock algorithm. It can't rely on the order of the locks to be the same across threads, so if it finds a lock that it can't acquire without blocking, it'll first release all of the locks it already acquired to avoid blocking other threads. This is wasteful because this algorithm may end up re-acquiring the same lock multiple times. To avoid this, ensure that (1) the first lock in the collection is always the first lock in any collection it appears in, and (2) the other locks in the collection are always preceded by that first lock. This will prevent any wasted time from re-acquiring locks. If you're unsure, [`LockCollection`] is a sensible default.

## Future Work

It might to possible to break the `ThreadKey` system by having two crates import this crate and call `ThreadKey::get`. I'm not quite sure how this works, but Rust could decide to give each crate their own key, ergo one thread would get two keys. I don't think the standard library would have this issue. At a certain point, I have to recognize that someone could also just import the standard library mutex and get a deadlock that way.

Are the ergonomics here any good? This is completely uncharted territory. Maybe there are some useful helper methods we don't have here yet. Maybe `try_lock` should return a `Result`. Maybe `lock_api` or `spin` implements some useful methods that I kept out for this proof of concept. Maybe there are some lock-specific methods that could be added to `LockCollection`. More types might be lockable using a lock collection.

It'd be nice to be able to use the mutexes built into the operating system, saving on binary size. Using `std::sync::Mutex` sounds promising, but it doesn't implement `RawMutex`, and implementing that is very difficult, if not impossible. Maybe I could implement my own abstraction over the OS mutexes. I could also simply implement `Lockable` for the standard library mutex.

Personally, I don't like mutex poisoning, but maybe it can be worked into the library if you're into that sort of thing.

It'd be interesting to add some methods such as `lock_clone` or `lock_swap`. This would still require a thread key, in case the mutex is already locked. The only way this could be done without a thread key is with a `&mut Mutex<T>`, but we already have `as_mut`. A `try_lock_clone` or `try_lock_swap` might not need a `ThreadKey` though. A special lock that looks like `Cell` but implements `Sync` could be shared without a thread key, because the lock would be dropped immediately (preventing non-preemptive allocation). It might make some common operations easier.

Now that we have the `Sharable` trait, indicating that all of the locks in a collection can be shared, we could implement a `Readonly` wrapper around the collections that don't allow access to `lock` and `try_lock`. The idea would be that if you're not exclusively locking the collection, then you don't need to check for duplicates in the collection. Calling `.read()` on the same `RwLock` twice dooes not cause a deadlock.

I want to try to get this working without the standard library. There are a few problems with this though. For instance, this crate uses `thread_local` to allow other threads to have their own keys. Also, the only practical type of mutex that would work is a spinlock. Although, more could be implemented using the `RawMutex` trait. The `Lockable` trait requires memory allocation at this time in order to check for duplicate locks.

I've been thinking about addiung `Condvar` and `Barrier`, but I've been stopped by two things. I don't use either of those very often, so I'm probably not the right person to try to implement either of them. They're also weird, and harder to prevent deadlocking for. They're sort of the opposite of a mutex, since a mutex guarantees that at least one thread can always access each resource.

Do `OnceLock` or `LazyLock` ever deadlock? We might not need to add those here.

We could implement special methods for something like a `LockCollection<Vec<i32>>` where we only lock the first three items.
