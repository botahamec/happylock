---
marp: true
theme: gaia
class: invert
---

<!-- _class: lead invert -->

# HappyLock

deadlock-free mutexes at compile-time

---

## Four Conditions for Deadlock

1. Mutual Exclusion
2. Non-preemptive Allocation
3. Cyclic Wait
4. Partial Allocation

---

## Preventing Mutual Exclusion

Mutual exclusion is the entire point of a mutex.

Do you want a `ReadOnly<T>` type?

Just use `Arc<T>` or `&T`!

---

## Prevent Non-Preemptive Allocation

```rust
let mutex = Mutex::new(10);
let mut number = mutex.lock();

let th = thread::spawn(|| {
    let number = mutex.lock(); // preempts the other lock on number
});
th.join();

prinln!("Thread 1: {}", *number); // oops, we don't have access to number anymore!
```

---

## Preventing Cyclic Wait

The language needs to enforce that all locks are acquired in the same order.

Rust doesn't have a built-in mechanism which can provide this.

Even if it could keep the locks in a certain order, using a `OrderedLock` type, we wouldn't be able to force you to use the mechanism.

And you could create two `OrderedLock` types and get deadlock using that.

---

## Preventing Partial Allocation

The language needs to enforce *total allocation*.

Acquiring a new lock requires releasing all currently-held locks.

**This will be our approach for now.**

---

## Quick Refresh on Borrow Checker Rules

1. You may have multiple immutable references to a value at a time
2. If there is a mutable reference to a value, then it is the only reference
3. Values cannot be moved while they are being referenced

```rust
let s = String::new("Hello, world!");
let r1 = &s;
let r2 = &s; // this is allowed because of #1
let mr = &mut s; // illegal: rule #2
drop(s); // also illegal: rule #3
println!("{r1} {r2}");
```

---

## How could an Operating System do this?

```c
#include <oslock.h>

void main() {
    os_mutex_t m = os_mutex_create();

    // the os returns an error if the rules aren't followed
    if (os_mutex_lock(&m)) {
        printf("Error!\n");
    }

    return 0;
}
```

---

## We have technology! (the borrow checker)

```rust
use happylock::{ThreadKey, Mutex};

fn main() {
    // each thread can only have one thread key (that's why we unwrap)
    // ThreadKey is not Send, Sync, Copy, or Clone
    let key = ThreadKey::get().unwrap();

    let mutex = Mutex::new(10);

    // locking a mutex requires either the ThreadKey or a &mut ThreadKey
    let mut guard = mutex.lock(key);
    // this means that a thread cannot lock more than one thing at a time

    println!("{}", *guard);
}
```

---

## Performance: it's freaking fast

`ThreadKey` is a mostly zero-cosst abstraction. It takes no memory at runtime. The only cost is getting and dropping the key.

`Mutex` is a thin wrapper around `parking_lot`. There's also a `spin` backend if needed for some reason.

---

## Wait, I need two mutexes

```rust
use happylock::{ThreadKey, Mutex, LockCollection};

fn main() {
    let key = ThreadKey::get().unwrap();
    let mutex1 = Mutex::new(5);
    let mutex2 = Mutex::new(String::new());

    let collection = LockCollection::new((mutex1, mutex2));
    let guard = collection.lock(key);

    *guard.1 = format!("{}{}", *guard.1, guard.0);
    *guard.0 += 1;
}
```

---

## The Lockable API

```rust
unsafe trait Lockable {
    type Guard;

    unsafe fn lock(&self) -> Self::Guard;

    unsafe fn try_lock(&self) -> Option<Self::Guard>;
}
```

---

## That's cool! Lemme try something

```rust
use happylock::{ThreadKey, Mutex, LockCollection};

fn main() {
    let key = ThreadKey::get().unwrap();
    let mutex1 = Mutex::new(5);

    // oh no. this will deadlock us
    let collection = LockCollection::new((&mutex1, &mutex1));
    let guard = collection.lock(key);

    // the good news is: this doesn't compile
}
```

---

## LockCollection's stub

```rust
impl<L: OwnedLockable> LockCollection<L> {
    pub fn new(data: L) -> Self { /***/ }
}

impl<L: OwnedLockable> LockCollection<&L> {
    pub fn new_ref(data: &L) -> Self { /***/ }
}

impl<L: Lockable> LockCollection<L> {
    // checks for duplicates
    pub fn try_new(data: L) -> Option<Self> { /***/ }

    pub unsafe fn new_unchecked(data: L) -> Self { /***/ }
}
```

---

## Changes to Lockable

```rust
unsafe trait Lockable {
    // ...

    fn get_ptrs(&self) -> Vec<usize>;
}



// not implemented for &L
// ergo: the values within are guaranteed to be unique
unsafe trait OwnedLockable: Lockable {}


```

---

## `contains_duplicates` (1st attempt)

```rust
fn contains_duplicates<L: Lockable>(data: L) -> bool {
    let pointers = data.get_ptrs();
    for (i, ptr1) in pointers.iter().enumerate() {
        for ptr2 in pointers.iter().take(i) {
            if ptr1 == ptr2 {
                return true;
            }
        }
    }

    false
}
```

Time Complexity: O(n²)

---

## 2nd attempt: sorting the pointers

```rust
fn contains_duplicates<L: Lockable>(data: L) -> bool {
    let mut pointers = data.get_ptrs();
    pointers.sort_unstable();
    pointers.windows(2).any(|w| w[0] == w[1])
}
```

Time Complexity: O(nlogn)

---

## Missing Features

- `Condvar`/`Barrier`
- We probably don't need `OnceLock` or `LazyLock`
- Standard Library Backend
- Mutex poisoning
- Support for `no_std`
- Convenience methods: `lock_swap`, `lock_set`?
- `try_lock_swap` doesn't need a `ThreadKey`
- Going further: `LockCell` API (preemptive allocation)

---

<!--_class: invert lead -->

## What's next?

---

## Problem: Live-locking

Although this library is able to successfully prevent deadlocks, livelocks may still be an issue. Imagine thread 1 gets resource 1, thread 2 gets resource 2, thread 1 realizes it can't get resource 2, thread 2 realizes it can't get resource 1, thread 1 drops resource 1, thread 2 drops resource 2, and then repeat forever. In practice, this situation probably wouldn't last forever. But it would be nice if this could be prevented somehow.

---

## Solution: Switch to preventing cyclic wait

- We're already sorting the pointers by memory address.
- So let's keep that order!

---

## Problems with This Approach

- I can't sort a tuple
  - Don't return them sorted, silly
- Indexing the locks in the right order
  - Have `get_ptrs` return a `&dyn Lock`
  - Start by locking everything
  - Then call a separate `guard` method to create the guard

---

# New traits

```rust
unsafe trait Lock {
    unsafe fn lock(&self);
    unsafe fn try_lock(&self) -> bool;
    unsafe fn unlock(&self);
}

unsafe trait Lockable {  // this is a bad name (LockGroup?)
    type Guard<'g>;
    fn get_locks<'a>(&'a self, &mut Vec<&'a dyn Lock>);
    unsafe fn guard<'g>(&'g self) -> Self::Guard<'g>;
}
```

---

## Ok let's get started, oh wait

- self-referential data structures
  - for best performance: `RefLockCollection`, `BoxedLockCollection`, `OwnedLockCollection`
- `LockCollection::new_ref` doesn't work without sorting
  - So as a fallback, provide `RetryingLockCollection`. It doesn't do any sorting, but unlikely to ever acquire the lock

---

<!--_class: invert lead -->

## The End
