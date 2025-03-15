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

<!-- _class: lead invert -->
# Background

---

## Goals: Background

 - Quick refresh on the borrow checker
 - What is a Mutex?
 - How Rust does Mutexes
 - What is a deadlock?
 - How can deadlocks be prevented?

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

## What is a Mutex?

It gives mutually-exclusive access for one thread, to prevent races.

```c
static pthread_mutex_t mutex = PTHREAD_MUTEX_INIT;
static int number = 1;

void thread_1() {
    pthread_mutex_lock(&mutex);
    number = 6;
    pthread_mutex_unlock(&mutex);
}
void thread_2() {
    pthread_mutex_lock(&mutex);
    printf("%d", number);
    pthread_mutex_unlock(&mutex);
}
```

This prevents the number from being modified while it is being printed, which would cause undefined behavior.

---

## Rust Mutexes

In Rust, the mutex is safer than in C. The mutex protects the data itself rather than sections of code.

```rust
static NUMBER: Mutex<i32> = Mutex::new(1);

fn thread_1() {
    // MutexGuard grants access to the data inside of the mutex
    // We cannot access this data without locking first
    let mut number: MutexGuard<'_, i32> = NUMBER.lock().unwrap();
    // MutexGuard is a smart pointer that we can modify directly
    *number += 5;

    // when the MutexGuard goes out of scope, it unlocks the mutex for you
}
```

---

## What is a deadlock?

Locking a mutex in a way that makes it impossible to be unlocked.

A simple way to cause deadlock is to lock twice on the same thread.

```rust
let number = Mutex::new(1);
let guard1 = number.lock().unwrap();
// now everybody has to wait until guard1 is dropped

let guard2 = number.lock().unwrap(); // but wait, guard1 still exists
// and we can't drop guard1, because we have to wait for this to finish
// THIS IS A DEADLOCK! (in C, this causes undefined behavior)

// we'll never get to do this
println!("{guard1} {guard2}");
```

---

## The Dining Philosopher's Problem

This is another example of deadlock, which is in the category of "deadly embrace"

<img src="https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Ffiles.codingninjas.in%2Farticle_images%2Fdining-philosopher-problem-using-semaphores-1-1643507259.jpg&f=1&nofb=1&ipt=d8d17865fc8cb4c2e66454e6833d43e83f3965573786c2c54cff601bae03e8a3&ipo=images" height="480" />

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

## Summary: Background

 - Mutexes allow mutually exclusive access to memory
 - Rust mutexes use the borrow checker to protect data, rather than sections of code
 - The borrow checker ensures the mutex is used somewhat correctly
 - Deadlocks prevent threads from making progress because a lock prevents unlocking
 - We can prevent deadlocks using total allocation

---

# Preventing Deadlock

---

## Goals: Preventing Deadlock

 - Show how the borrow checker can enforce total allocation
 - Lock multiple mutexes at the same time
 - Make sure that we lock multiple mutexes, we don't lock the same one twice

---

## We have technology! (the borrow checker)

```rust
use happylock::{ThreadKey, Mutex};

fn main() {
    // each thread can only have one thread key (that's why we unwrap)
    // ThreadKey is not Send, Copy, or Clone
    let key = ThreadKey::get().unwrap();

    let mutex = Mutex::new(10);

    // locking a mutex requires the ThreadKey
    let mut guard = mutex.lock(key);
    // this means that a thread cannot lock more than one thing at a time

    println!("{}", *guard);

    // you can get the ThreadKey back by unlocking
    let key = Mutex::unlock(guard);
}
```

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

## Summary: Preventing Deadlock

 - Using an owned `ThreadKey` type, we can ensure total allocation
 - Using a `LockCollection`, we can lock multiple mutexes at the same time
 - The marker trait, `OwnedLockable`, can be guaranteed to not contain duplicates at compile-time
 - We can check for duplicates at runtime by checking the memory addresses of the locks

---

<!-- _class: lead invert -->
# Optimizations to HappyLock

---

## Goals: Optimizations

 - Explain how exactly we check for duplicates
 - Naively implement `LockCollection`
 - Understand how live-locking hurts total allocation
 - Rewrite our collections to prevent cyclic wait, which will improve performance
 - Provide a way to create a user-defined locking order

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

## RetryingLockCollection

```rust
struct RetryingLockCollection<L> {
    data: L,
}
```

This will try to lock everything in the collection, and release everything if it fails.

We keep trying until we've locked everything in a row.

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

# New traits

```rust
unsafe trait RawLock {
    unsafe fn lock(&self);
    unsafe fn try_lock(&self) -> bool;
    unsafe fn unlock(&self);
}

unsafe trait Lockable {  // this is a bad name (LockGroup?)
    type Guard<'g> where Self: 'g;
    fn get_locks<'a>(&'a self, &mut Vec<&'a dyn RawLock>);
    unsafe fn guard<'g>(&'g self) -> Self::Guard<'g>;
}
```

---

## Solving self-referential data structures with heap allocation

```rust
struct BoxedLockCollection<L> {
    data: *const UnsafeCell<L>,
    locks: Vec<&'static dyn RawLock>
}
```

This is the default lock collection in HappyLock

---

## Providing our own lock ordering

 - What if we don't want to do any sorting?
 - We could make a collection that allows us to define our own order
 - This requires that no other ordering is used for the locks in that collection
 - `OwnedLockable` can be used for this

---

## Solving self-referential data structures with owned lockables

```rust
struct OwnedLockCollection<L: OwnedLockable> {
    data: L,
}
```

---

## Summary: Optimizations

 - Livelocking causes perpetual locking and unlocking and retries
 - Locks are sorted by memory address before looking for duplicates
 - `BoxedLockCollection` sorts in the order of the memory address
 - `OwnedLockCollection` allows us to define our own lock order
 - `RetryingLockCollection` has the original retry behavior

---

<!-- _class: lead invert -->
# Quality of Life Enhancements

---

## Goals: Quality of Life

 - Try to lock a mutex using a `&mut ThreadKey`
 - Take inspiration from scoped threads to ensure unlocking
 - Use marker traits to allow read-only locks
 - Use traits to implement `get_mut` and `into_inner`

---

## Keyable

```rust
unsafe trait Keyable: Sealed {}
unsafe impl Keyable for ThreadKey {}
unsafe impl Keyable for &mut ThreadKey {}
```

This is helpful because you can get the thread key back immediately.

```rust
impl<T, R> Mutex<T, R> {
    pub fn lock<'a, 'key, Key: Keyable + 'key>(
        &'a self,
        key: Key
    ) -> MutexGuard<'a, 'key, T, R, Key>;
}
```

---

## Keyable

So conveniently, this compiles.

```rust
let mut key = ThreadKey::get().unwrap();
let guard = MUTEX1.lock(&mut key);

// the first guard can no longer be used here
let guard = MUTEX1.lock(&mut key);
```

---

## Keyable

The problem is that this also compiles

```rust
let guard = MUTEX1.lock(&mut key);
std::mem::forget(guard);

// wait, the mutex is still locked!
let guard = MUTEX1.lock(&mut key);
// deadlocked now
```

---

## Scoped Threads

Let's take inspiration from scoped threads:

```rust
fn scope<'env, F, T>(f: F) -> T
where
    F: for<'scope> FnOnce(&'scope Scope<'scope, env>) -> T;
```

The `Drop` implementation of the `Scope` type will join all of the spawned threads. And because we only have a  reference to the `Scope`, we'll never be able to `mem::forget` it.

---

## Scoped Threads

```rust
let mut a = vec![1, 2, 3];
let mut x = 0;

scope(|scope| {
    scope.spawn(|| {
        println!("we can borrow `a` here");
        dbg!(a)
    });
    scope.spawn(|| {
        println!("we can even borrow mutably");
        println!("no other threads will use it");
        x += a[0] + a[2];
    });
    println!("hello from the main thread");
});
```

---

## Scoped Locks

Let's try the same thing for locks

```rust
let mut key = ThreadKey::get().unwrap();
let mutex_plus_one = MUTEX.scoped_lock(&mut key, |guard: &mut i32| *guard + 1);
```

If you use scoped locks, then you can guarantee that locks will always be unlocked (assuming you never immediately abort the thread).

---

## Allowing reads on `LockCollection`

```rust
// update RawLock
unsafe trait RawLock {
    // * snip *
    unsafe fn read(&self);
    unsafe fn try_read(&self);
    unsafe fn unlock_read(&self);
}

// This trait is used to indicate that reading is actually useful
unsafe trait Sharable: Lockable {
    type ReadGuard<'g> where Self: 'g;

    unsafe fn read_guard<'g>(&'g self) -> Self::ReadGuard<'g>;
}
```

---

## Not every lock can be read tho

```rust
impl<L: Sharable> LockCollection<L> {
    pub fn read<..>(&'g self, key: Key) -> LockGuard<..> { /* ... */ }

    pub fn try_read<..>(&'g self, key: Key) -> Option<LockGuard<..>> { /* ... */ }

    pub fn unlock_read<..>(guard: LockGuard<..>) { /* ... */ }
}
```

---

## `LockableGetMut`

```rust
fn Mutex::<T>::get_mut(&mut self) -> &mut T // already exists in std
// this is safe because a mutable reference means nobody else can access the lock

trait LockableGetMut: Lockable {
    type Inner<'a>;

    fn get_mut(&mut self) -> Self::Inner<'_>
}

impl<A: LockableGetMut, B: LockableGetMut> LockableGetMut for (A, B) {
    type Inner<'a> = (A::Inner<'a>, B::Inner<'b>);

    fn get_mut(&mut self) -> Self::Inner<'_> {
        (self.0.get_mut(), self.1.get_mut())
    }
}
```
---

## Summary: QoL Enhancements

 - Using `&mut ThreadKey` won't work because someone could `mem::forget` a lock guard
 - To guarantee unlocking, we can use a scoped API
 - Marker traits can be used to indicate that a lock can be shared
 - Traits can also provide other functionality, like `get_mut`

---

<!-- _class: lead invert -->
# The Future: Expanding Cyclic Wait

---

## Goals: Expanding Cyclic Wait

 - Show that we sometimes need partial allocation
 - Idealize how cyclic wait could be used in these scenarios
 - Design an API that could use typestate to support cyclic wait and partial allocation
 - Ensure that the `ThreadKey` is not used while multiple partially allocated guards are active

---

## Expanding Cyclic Wait

> ... sometimes you need to lock an object to read its value and determine what should be locked next... is there a way to address it?

```rust
let guard = m1.lock(key);
if *guard == true {
    let key = Mutex::unlock(guard);
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
let (guard, next: LockIterator<Vec<String>>) = iterator.next();

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

The `ThreadKey` needs to be held somewhere while the guards are active.

**How do we ensure that the `ThreadKey` is not used again until all of the guards are dropped?**

---

## The Solution

First, every guard needs to have an immutable reference to the `ThreadKey`.

```rust
// this is the MutexGuard that doesn't hold a ThreadKey
// We'll modify it to hold an immutable reference to the ThreadKey
// ThreadKey cannot be moved or mutably referenced during this lifetime
struct MutexRef<'a, 'key, T, Key: Keyable + 'key>
struct RwLockReadRef<'a, 'key, T, Key: Keyable + 'key>
struct RwLockWriteRef<'a, 'key, T, Key: Keyable + 'key>
```

---

## The Solution

But where do we store the `ThreadKey`?

```rust
// This type will hold the ThreadKey
struct LockIteratorGuard<'a, L> {
    collection: &'a OwnedLockCollection<L>,
    thread_key: ThreadKey,
}
```

---

## The Solution

Then `LockIterator` must hold a reference to the guard.

```rust
struct LockIterator<'a, Current, Rest = ()>
```

---

## The Solution

And we can get the first LockIterator by taking a mutable reference to the guard.

```rust
LockIteratorGuard::next<'a>(&'a mut self) -> LockIterator<'a, L::Next>
```

---

## Summary: Expanding Cyclic Wait

 - Partial allocation is needed in situations like skip lists
 - Cyclic wait can be used as a backup in these situations
 - Typestate allows us to iterate over the elements of a tuple
 - A `ThreadKey` must be stored somewhere while the partially allocated guards are active
 - Immutable references to the `ThreadKey` can prove that the `ThreadKey` is not used until *all* of the guards are dropped

---

<!--_class: invert lead -->
## The End
