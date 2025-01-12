---
marp: true
theme: gaia
class: invert
author: Mica White
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
    // ThreadKey is not Send, Copy, or Clone
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

This `LockCollection` can be implemented simply by releasing the currently acquired locks and retrying on failure

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

## Problem: Live-locking

Although this library is able to successfully prevent deadlocks, livelocks may still be an issue.

1. Thread 1 locks mutex 1
2. Thread 2 locks mutex 2
3. Thread 1 tries to lock mutex 2 and fails
4. Thread 2 tries to lock mutex 1 and fails
5. Thread 1 releases mutex 1
6. Thread 2 releases mutex 2
7. Repeat

This pattern will probably end eventually, but we should really avoid it, for performance reasons.

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
unsafe trait RawLock {
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

## Solving self-referential data structures with references

```rust
struct RefLockCollection<'a, L> {
    data: &'a L,
    locks: Vec<&'a dyn RawLock>
}
```

But what if I don't want to manage lifetimes?

---

## Solving self-referential data structures with heap allocation

```rust
struct BoxedLockCollection<L> {
    data: *const UnsafeCell<L>,
    locks: Vec<&'static dyn RawLock>
}
```

But what if I don't want to do any lock sorting?

---

## Solving self-referential data structures with owned lockables

```rust
struct OwnedLockCollection<L> {
    data: L,
}
```

This doesn't allow immutable references to data, but also doesn't require any sorting

But what if I don't have ownership of the locks?

---

## Solving self-referential data structures with retrying locks

```rust
struct RetryingLockCollection<L> {
    data: L,
}
```

This is what we were trying to avoid earlier

---

## Let's make four different lock collection types that do almost the same thing!

- `BoxedLockCollection`: The default. Sorts the locks by memory address, and locks in that order. Heap allocation is required.

- `RefLockCollection`: Same as boxed, but requires the caller to manage the lifetimes.

- `RetryingLockCollection`: Mostly cheap to construct, but will usually fail to lock the data if there is any contention.

- `OwnedLockCollection`: The cheapest option. Locks in the order given. No shared references to inner collection.

---

## RwLocks in collections

This is what I used in HappyLock 0.1:

```rust
struct ReadLock<'a, T>(&'a RwLock<T>);
struct WriteLock<'a, T>(&'a RwLock<T>);
```

**Problem:** This can't be used inside of an `OwnedLockCollection`

---

## Allowing reads on `OwnedLockCollection`

```rust
// update RawLock
unsafe trait RawLock {
    // * snip *
    unsafe fn read(&self);
    unsafe fn try_read(&self);
    unsafe fn unlock_read(&self);
}

// update Lockable
unsafe trait Lockable {
    // * snip *
    type ReadGuard<'g> where Self: 'g;
    unsafe fn read_guard<'g>(&'g self) -> Self::ReadGuard<'g>;
}
```

---

## Not every lock can be read tho

```rust
// This trait is used to indicate that reading is actually useful
unsafe trait Sharable: Lockable {}

impl<L: Sharable> OwnedLockable<L> {
    pub fn read<..>(&'g self, key: Key) -> LockGuard<..> { /* ... */ }

    pub fn try_read<..>(&'g self, key: Key) -> Option<LockGuard<..>> { /* ... */ }

    pub fn unlock_read<..>(guard: LockGuard<..>) { /* ... */ }
}

// the same methods exist on other lock collections too
```

---

## Poisoning

```rust
unsafe impl<L: Lockable + Send + Sync> RawLock for LockCollection<L>

pub struct Poisonable<L: Lockable + RawLock> {
    inner: L,
    poisoned: PoisonFlag,
}

impl<L: Lockable + RawLock> Lockable for Poisonable<L> {
    type Guard<'g> = Result<PoisonRef<'g, L::Guard>, PoisonErrorRef<'g, L::Guard>>
        where Self: 'g;

    // and so on...
}
```

Allows: `Poisonable<LockCollection>` and `LockCollection<Poisonable>`

---

# `LockableGetMut` and `LockableIntoInner`

```rust
fn Mutex::<T>::get_mut(&mut self) -> &mut T // already exists in std
// this is safe because a mutable reference means nobody else can access the lock

trait LockableGetMut: Lockable {
    type Inner<'a>;

    fn get_mut(&mut self) -> Self::Inner<'_>
}

impl<A: LockableGetMut, B: LockableGetMut> LockableGetMut for (A, B) {
    type Inner = (A::Inner<'a>, B::Inner<'b>);

    fn get_mut(&mut self) -> Self::Inner<'_> {
        (self.0.get_mut(), self.1.get_mut())
    }
}
```

---

## Missing Features

- `Condvar`/`Barrier`
- `OnceLock` or `LazyLock`
- Standard Library Backend
- Support for `no_std`
- Convenience methods: `lock_swap`, `lock_set`?
- `try_lock_swap` doesn't need a `ThreadKey`
- Going further: `LockCell` API (preemptive allocation)

---

<!--_class: invert lead -->

## What's next?

---


## OS Locks

- Using `parking_lot` makes the binary size much larger
- Unfortunately, it's impossible to implement `RawLock` on the standard library lock primitives
- Creating a new crate based on a fork of the standard library is hard
- Solution: create a new library (`sys_locks`), which exposes raw locks from the operating system
- This is more complicated than you might think

---

## Compile-Time Duplicate Checks

As Mikhail keeps reminding me, it might be possible to do the duplicate detection at compile-time using a Bloom filter. This is something I'll have to try at some point.

This would only be useful for `RetryingLockCollection`

---

# Convenience Methods

`Mutex::lock_swap`, `lock_clone`, etc would be cool

\
\
\
These methods would still require a `ThreadKey` though

```rust
let guard = mutex.lock(key);
let cloned = mutex.lock_clone(); // deadlock
```

---

# Try-Convenience Methods

- `Mutex::try_swap`, `try_set` would not require a `ThreadKey`
- They can't block the current thread (because `try`), and they can't block other threads (because they release the lock immediately)
- Same probably applies to `try_clone` and `try_take`, but could be a problem if the `Clone` implementation tries to lock something else

---

## `LockCell`

This would only allow methods tyo be called which immediately release the lock

This would never require a `ThreadKey`

Could run into bugs:

```rust
let a: LockCell<i32>;
let b: LockCell<i32>;
if a == b {
    // a and b are no longer equal
}
```

---

## Readonly Lock Collections

- The original idea behind `Sharable` was to avoid duplicate checking on collections that will only ever be read.
- Reading can't cause a deadlock? Or so I thought.

From the standard library documentation:

```txt
// Thread 1              |  // Thread 2
let _rg1 = lock.read();  |
                         |  // will block
                         |  let _wg = lock.write();
// may deadlock          |
let _rg2 = lock.read();  |
```

---

## Recursive Trait

Instead of `Sharable`, use a `Recursive` trait:

```rust
unsafe trait Recursive: Lockable {}
```

Add a `Readonly` wrapper around collections of recursives:

```rust
pub struct Readonly<L: Recursive> {
    inner: L
}
```

A `Readonly` collection cannot be exclusively locked.

---

## Others

- No standard library
  - hard, because of thread local, and allocation being required for lock collections
- Condvar and Barrier
  - these have completely different deadlocking rules. someone else should figure this out
- LazyLock and OnceLock
  - can these even deadlock?

---
## Expanding Cyclic Wait

> ... sometimes you need to lock an object to read its value and determine what should be locked next... is there a way to address it?

```rust
let guard = m1.lock(key);
if *guard == true {
    let key = Mutex::unlock(m);
    let data = [&m1, &m2];
    let collection = LockCollection::try_new(data).unwrap();
    let guard = collection.lock(key);

    // m1 might no longer be true here...
}
```

---

## What I Really Want

```txt
ordered locks: m1, m2, m3

if m1 is true
    lock m2 and keep m1 locked
else
    skip m2 and lock m3
```

We can specify lock orders using `OwnedLockCollection`

Then we need an iterator over the collection to keep that ordering

This will be hard to do with tuples (but is not be impossible)

---

## Something like this

```rust
let key = ThreadKey::get().unwrap();
let collection: OwnedLockCollection<(Vec<i32>, Vec<String>);
let iterator: LockIterator<(Vec<i32>, Vec<String>)> = collection.locking_iter(key);
let (guard, next: LockIterator<Vec<String>>) = collection.next();

unsafe trait IntoLockIterator: Lockable {
    type Next: Lockable;
    type Rest;

    unsafe fn next(&self) -> Self::Next; // must be called before `rest`
    fn rest(&self) -> Self::Rest;
}

unsafe impl<A: Lockable, B: Lockable> IntoLockIterator for (A, B) {
    type Next = A;
    type Rest = B;

    unsafe fn next(&self) -> Self::Next { self.0 }

    unsafe fn rest(&self) -> Self::Rest { self.1 }
}
```

---

## Here are the helper functions we'll need

```rust
struct LockIterator<Current: IntoLockIterator, Rest: IntoLockIterator = ()>;

impl<Current, Rest> LockIterator<Current, Rest> {
    // locks the next item and moves on
    fn next(self) -> (Current::Next::Guard, LockIterator<Current::Rest>);

    // moves on without locking anything
    fn skip(self) -> LockIterator<Current::Rest>;

    // steps into the next item, allowing parts of it to be locked
    // For example, if i have LockIterator<(Vec<String>, Vec<i32>)>, but only
    // want to lock parts of the first Vec, then I can step into it,
    // locking what i need to, and then exit.
    // This is the first use of LockIterator's second generic parameter
    fn step_into(self) -> LockIterator<Current::Next, Rest=Current::Rest>;

    // Once I'm done with my step_into, I can leave and move on
    fn exit(self) -> LockIterator<Rest>;
}
```

---

## A Quick Problem with this Approach

We're going to be returning a lot of guards.

The `ThreadKey` is held by the `LockIterator`.

**How do we ensure that the `ThreadKey` is not used again until all of the guards are dropped?**

---

<!--_class: invert lead -->

## The End
