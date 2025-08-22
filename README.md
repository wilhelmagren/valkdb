## valkdb

This is a project that follows the tokio guide for creating our own **mini-redis**.


### What is asynchronous programming?

Most computer programs are executed in the same order in which they are written.
The first line executes, then the next, and so on. With synchronous programming,
when a program encounters an operation that cannot be completed immediately,
it will block until the operation completes. For example, establishing a TCP
connection requires an exchange with a peer over the network, which can take
a sizeable amount of time. During this time, the thread is blocked.

With asynchronous programming, operations that cannot be completed immediately
are suspended to the background. The thread is not blocked, and can continue
running other things. Once the operation completes, the task is unsuspended
and continues processing from where it left off.

Although asynchronous programming can result in faster applications, it often
results in much more complicated programs. The programmer is required to track
all the state necessary to resume work once the asynchronous operation completes.
Historically, this is a tedious and error-prone task.


### Compile-time green-threading

> In computer programming, a green thread is a thread that is scheduled by a runtime
> library or virtual machine instead of natively by the underlying operating system.
> Green threads emulate multithreaded environments without relying on any native OS
> abilities, and they are managed in user space instead of kernel space, enabling them
> to work in environments that do not have native thread support.*

Rust implements asynchronous programming using a feature called `async/await`.
Functions that perform asynchronous operations are labeled with the `async` keyword.
In our example, the `connect` function is defined as:

```rust
use mini_redis::Result;
use mini_reids::client::Client;
use tokio::net::ToSocketAddrs;

pub async fn connect<T: ToSocketAddrs>(addr: T) -> Result<Client> {
    // ...
}
```

The `async fn` definition looks like a regular synchronous function, but operates
asynchronously. Rust transforms the `async fn` at **compile** time into a routine
that operates asynchronously. Any calls to `.await` within the `async fn` yield
control back to the thread. The thread may do other work wile the operation
processes in the background.

The async function

```rust
async fn my_fn<T>(t: T) -> T {
    todo!()
}
```

roughly desugars into

```rust
fn my_fn<T>(t: T) -> impl Future<Output = T> {
    todo!()
}
```

and a `Future` represents some form of asynchronous computation that we can
`await` to get the result of.

### Using `async/await`

Async functions are called like any other Rust function. However, calling these
functions does not result in the function body executing. Instead, calling an
`async fn` returns a value representing the operation. This is conceptually
analogous to a zero-argument closure. To actually run the operation, you should
use the `.await` operator on the returned value.

For example, the given program

```rust
async fn say_world() {
    println!("world");
}

#[tokio::main]
async fn main() {
    // Calling `say_world()` does not execute the body of the function.
    let op = say_world();

    println!("Hello");

    op.await;

    // Now we will have printed first 'Hello' and then 'world'.
}
```

The return value of an `async fn` is an anonymous type that implements the `Future` trait.


### Async `main` function

The main function used to launch the application differs from the usual one found in most
of Rust's crates.
1. It is an `async fn`
2. It is annotated with `#[tokio::main]`

An `async fn` is used as we want to enter an asynchronous context. However, asynchronous
functions must be executed by a **runtime**. The runtime contains the asynchronous task
scheduler, provides evented I/O timers, etc. The runtime does not start automatically,
so the main function needs to start it.

The `#[tokio::main]` function is a macro. It transforms the `async fn main()` into
a synchronous `fn main()` that initializes a runtime instance and executes the async
main function.

For example, the following:

```Rust
#[tokio::main]
async fn main() {
    println!("hello");
}
```

gets transformed into:

```rust
fn main() {
    let mut rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        println!("hello");
    })
}
```


### Concurrency

Our server has a slight problem (besides only responding with errors). It processes
inbound requests one at a time. When a connection is accepted, the server stays inside
the accept loop block until the repsonse is fully written to the socket.

We want our Redis server to process **many** concurrent requests. To do this we need
to add some concurrency.

> Concurrency and parallelism are not the same thing. If you alternate between two tasks,
> then you are working on both tasks concurrently, but no in parallel. For it to qualify
> as parallel, you would need two people, one dedicated to each task.

> One of the advantages of using Tokio is that asynchronous code allows you to work on
> many tasks concurrently, without having to work on them in parallel using ordinary
> threads. In fact, Tokio can run many tasks concurrently on a single thread!

To process connections concurrently, a new task is spawned for each inbound connection.
The connection is processed on this task.

The accept loop becomes:

```rust
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        // A new task is spawned for each inbound socket. The socket
        // is moved to the new task and processed there.
        tokio::spawn(async move {
            process(socket).await;
        })
    }
}
```

### Tasks

A Tokio task is an asynchronous green thread. They are created by passing an
`async` block to `tokio::spawn`. The `tokio::spawn` function returns a `JoinHandle`,
which the caller may use to interact with the spawned task. The `async` block may
have a return value. The caller may obtain the return value using `.await` on
the `JoinHandle`.

For example:

```rust
#[tokio::main]
async fn main() {
    let handle = tokio::spawn(async {
        // Do some async work.
        "return value"
    });

    // Do some other work.
    // ...

    let out = handle.await.unwrap();
    println!("GOT {}", out);
}
```

Awaiting on a `JoinHandle` returns a `Result`. When a task encounters an error
during execution, the `JoinHandle` will return an `Err`. This happens when
the task either panics, or if the task is forcefully cancelled by the runtime
shutting down.

Tasks are the unit of execution managed by the scheduler. Spawning the task
submits it to the Tokio scheduler, which then ensures that the task executes
when it has work to do. The spanwed task may be executed on the same thread
as where it was spawned, or it may execute on a different runtime thread.
The task can also be moved between threads after being spawned.

Tasks in Tokio are very lighweight. Under the hood, they require only a single
allocation and 64 bytes of memory. Applications should feel free to spawn
thousands, if not millions of tasks.

### `'static` bound

When you spawn a task on the Tokio runtime, its type's lifetime must be `'static`.
This means that the spawned task must not contain any references to data owned
outside the task.

> It is a common misconception that `'static` always means "lives forever",
> but this is not the case. Just because a value is `'static` does not mean
> that you have a memory leak. Read more on [Common Rust Lifetime Misconceptions](https://github.com/pretzelhammer/rust-blog/blob/master/posts/common-rust-lifetime-misconceptions.md#2-if-t-static-then-t-must-be-valid-for-the-entire-program)

For example, the following will not compile:

```rust
use tokio::task;

#[tokio::main]
async fn main() {
    let v = vec![1, 2, 3];
    task::spawn(async {
        println!("Here's the vec: {:?}", v);
    });
}
```

Attempting to compile this results in the following error:

```
error[E0373]: async block may outlive the current function, but
              it borrows `v`, which is owned by the current function
 --> src/main.rs:7:23
  |
7 |       task::spawn(async {
  |  _______________________^
8 | |         println!("Here's a vec: {:?}", v);
  | |                                        - `v` is borrowed here
9 | |     });
  | |_____^ may outlive borrowed value `v`
  |
note: function requires argument type to outlive `'static`
 --> src/main.rs:7:17
  |
7 |       task::spawn(async {
  |  _________________^
8 | |         println!("Here's a vector: {:?}", v);
9 | |     });
  | |_____^
help: to force the async block to take ownership of `v` (and any other
      referenced variables), use the `move` keyword
  |
7 |     task::spawn(async move {
8 |         println!("Here's a vec: {:?}", v);
9 |     });
  |
```

This happens because, by default, variables are **not moved** into async blocks.
The `v` vector remains owned by the `main` function. The `println!` line borrows
`v`. The rust compiler helpfully explains this to us and even suggest the fix!
Changing line 7 to `task::spawn(async move {` will instruct the compiler to **move**
`v` into the spawned task. Now, the task owns all of its data, making it `'static`.

If a single piece of data must be accessible from more than one task concurrently,
then it must be shared using synchronization primitives such as `Arc`.

Note that the error message talks about the argument type *outliving* the `'static`
lifetime. This terminology can be rather confusing because the `'static` lifetime
lasts until the end of the program, so if it outlives it, don't you have a memory
leak? The explanation is that it is the *type*, not the *value* that must outlive
the `'static` lifetime, and the value may be destroyed before its type is no longer valid.

When we say that a value is `'static`, all that means is that it would be incorrect
to keep that value around forever. This is important because the compiler is unable
to reason about how long a newly spawned task stays around. We have to make sure that
the task is allowed to live forever, so that Tokio can make the task run as long as
it needs to.

The article that the info-box above links to uses the terminology "bounded by `'static`"
rather than "its type outlives `'static`" or "the value is `'static`" to refer to
`T: 'static`. These all mean the same thing, but are different from "annotated with
`'static`" as in `&'static T`.


### `Send` bound

Tasks spawned by `tokio::spawn` **must** implement `Send`. This allows the Tokio
runtime to move the tasks between threads while they are suspended at an `.await`.

Tasks are `Send` when **all** data that is held **across** `.await` calls is `Send`.
This is a bit subtle. When `.await` is called, the task yields back to the scheduler.
The next time the task is executed, it resumes from the point it last yielded. To make
this work, all state that is used **after** `.await` must be saved by the task. If this
state is `Send`, i.e. it can be moved across threads, then the task itself can be moved
across threads. Conversely, if the state is not `Send`, then neither is the task.

For example, this works:

```rust
use tokio::task::yield_now;
use std::rc::Rc;

#[tokio::main]
async fn main() {
    tokio::spawn(async {
        // The scope forces `rc` to drop before `.await`.
        {
            let rc = Rc::new("Hello");
            println!("{}", rc);
        }

        // `rc` is no longer used, it is **not** persisted when
        // the task yields to the scheduler.
        yield_now().await;
    });
}
```

This does not work:

```rust
use tokio::task::yield_now;
use std::rc::Rc;

#[tokio::main]
async fn main() {
    tokio::spawn(async {
        let rc = Rc::new("Hello");

        // `rc` is used after `.await` so it must be
        // persisted to the task's state.
        yield_now().await;

        println!("{}", rc);
    });
}
```

Attempting to compile the snippet results in:

```
error: future cannot be sent between threads safely
   --> src/main.rs:6:5
    |
6   |     tokio::spawn(async {
    |     ^^^^^^^^^^^^ future created by async block is not `Send`
    | 
   ::: [..]spawn.rs:127:21
    |
127 |         T: Future + Send + 'static,
    |                     ---- required by this bound in
    |                          `tokio::task::spawn::spawn`
    |
    = help: within `impl std::future::Future`, the trait
    |       `std::marker::Send` is not  implemented for
    |       `std::rc::Rc<&str>`
note: future is not `Send` as this value is used across an await
   --> src/main.rs:10:9
    |
7   |         let rc = Rc::new("hello");
    |             -- has type `std::rc::Rc<&str>` which is not `Send`
...
10  |         yield_now().await;
    |         ^^^^^^^^^^^^^^^^^ await occurs here, with `rc` maybe
    |                           used later
11  |         println!("{}", rc);
12  |     });
    |     - `rc` is later dropped here
```

because `Rc` does not implement `Send`.


### Shared state

So far, we have a key-value server working. However, there is a major flaw:
state is not shared across connections. Let us fix that!

There are a couple of ways to share state in Tokio.
1. Guard the shared state with a Mutex.
2. Spawn a task to manage the state and use message passing to operate on it.

Generally you want to use the first approach for simple data, and the second
approach for things that require asynchronous work such as I/O primitives.
The shared state we use is a `HashMap` and the operations are `insert` and
`get`. Neither of these operations is asynchronous, so we will use a `Mutex`.

Instead of using `Vec<u8>` we will use `Bytes` from the **bytes** crate. The
goal of `Bytes` is to provide a robust byte array strucute for network
programming. The biggest feature it adds over `Vec<u8>` is shallow cloning.
In other words, calling `clone()` on a `Bytes` instance does not copy the
underlying data. Instead, a `Bytes` instance is a refrence-counted handle
to some underlying data. The `Bytes` type is thus roughly an `Arc<Vec<u8>>`
but with some added capabilities.

The `HashMap` will be shared across many tasks and potentially many threads.
To support this, it is wrapped in `Arc<Mutex<_>>`.

```rust
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type Db = Arc<Mutex<HashMap<String, Bytes>>>;
```

Note that `std::sync::Mutex` and **not** `tokio::sync::Mutex` is used to guard
the `HashMap`. A common error is to unconditionally use `tokio::sync::Mutex` from
within async code. An async mutex is a mutex that is locked across calls to `.await`.


### Holding a `MutexGuard` across an `.await`

You might write code that looks like this:

```rust
use std::sync::{Mutex, MutexGuard};

async fn increment_and_do_stuff(mutex: &Mutex<i32>) {
    let mut lock: MutexGuard<i32> = mutex.lock().unwrap();
    *lock += 1;

    do_something_async().await;
} // lock goes out of scope here
```

When you try and spawn something that calls this function, you will encounter
the following error message:

```
error: future cannot be sent between threads safely
   --> src/lib.rs:13:5
    |
13  |     tokio::spawn(async move {
    |     ^^^^^^^^^^^^ future created by async block is not `Send`
    |
   ::: /playground/.cargo/registry/src/github.com-1ecc6299db9ec823/tokio-0.2.21/src/task/spawn.rs:127:21
    |
127 |         T: Future + Send + 'static,
    |                     ---- required by this bound in `tokio::task::spawn::spawn`
    |
    = help: within `impl std::future::Future`, the trait `std::marker::Send` is not implemented for `std::sync::MutexGuard<'_, i32>`
note: future is not `Send` as this value is used across an await
   --> src/lib.rs:7:5
    |
4   |     let mut lock: MutexGuard<i32> = mutex.lock().unwrap();
    |         -------- has type `std::sync::MutexGuard<'_, i32>` which is not `Send`
...
7   |     do_something_async().await;
    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^ await occurs here, with `mut lock` maybe used later
8   | }
    | - `mut lock` is later dropped here
```

This happens because the `std::sync::MutexGuard` type is **not** `Send`. This means that
you can't send a mutex lock to another thread, and the error happens because the Tokio
runtime can move a task between threads at every `.await`. To avoid this, you should
restructure your code such that the mutex lock's destructor runs before the `.await`.

```rust
async fn increment_and_do_stuff(mutex: &Mutex<i32>) {
    {
        let mut lock: MutexGuard<i32> = mutex.lock().unwrap();
        *lock += 1;
    } // lock goes out of scope here

    do_something_async().await;
}
```

Note that this does not work:

```rust
use std::sync::{Mutex, MutexGuard};

// This fails too.
async fn increment_and_do_stuff(mutex: &Mutex<i32>) {
    let mut lock: MutexGuard<i32> = mutex.lock().unwrap();
    *lock += 1;
    drop(lock);

    do_something_async().await;
}
```

This is because the compiler currently calculates whether a future is `Send` based
on scope information only. The compiler will hopefully be updated to support explicitly
dropping it in the future, but for now, you must explicitly use a scope.

The safest way to handle a mutex is to wrap it in a struct, and lock the mutex only
inside non-async methods on that struct.

```rust
use std::sync::Mutex;

struct CanIncrement {
    mutex: Mutex<i32>,
}

impl CanIncrement {
    fn increment(&self) {
        let mut lock = self.mutex.lock().unwrap();
        *lock += 1;
    }
}

async fn increment_and_do_stuff(can_incr: &CanIncrement) {
    can_incr.increment();
    do_something_async().await;
}
```

This pattern guarantees that you won't run into the `Send` error, because the mutex
guard does not appear anywhere in an async function. It also protects you from deadlocks,
when using crates whose `MutexGuard` implements `Send`.

The `tokio::sync::Mutex` type provided by Tokio can also be used. The primary feature of
the Tokio mutex is that it can be held across an `.await` without any issues. That said,
an asynchronous mutex is more expensive than an ordinary mutex, and it is typically better
to use one of the two other approaches.

```rust
use tokio::sync::Mutex;  // note! this uses the Tokio mutex

async fn increment_and_do_stuff(mutex: &Mutex<i32>) {
    let mut lock = mutex.lock().unwrap();
    *lock += 1;

    do_something_async().await;
} // lock goes out of scope here
```


### Tasks, threads, and contention

Using a blocking mutex to guard short critical sections is an acceptable strategy
when contention is minimal. When a lock is contended, the thread executing the task
must block and wait on the mutex. This will not only block the current task but it
will also block all other tasks scheduled on the current thread.

By default, the Tokio runtime uses a multi-threaded scheduler. Tasks are scheduled on
any number of threads managed by the runtime. If a large number of tasks are scheduled
to execute and they all require access to the mutex, then there will be contention.
On the other hand, if the `current_thread` runtime flavor is used, then the mutex will
never be contended.

If contention on a synchronization mutex becomes a problem, the best fix is rarely to
switch to the Tokio mutex. Instead, options to consider are to:
- Let a dedicated task manage the state and use message passing.
- Shard the mutex.
- Restructure the code to avoid the mutex.


### Mutex sharding

In our case, as each *key* is independent, mutex sharding will work well. To do this,
instead of having a single `Mutex<HashMap<_, _>>` instance, we would introduce `N`
distinct instances.

```rust
type ShardedDb = Arc<Vec<Mutex<HashMap<String, Vec<u8>>>>>;

fn new_sharded_db(num_shards: usize) -> ShardedDb {
    let mut db = Vec::with_capacity(num_shards);
    for _ in 0..num_shards {
        db.push(Mutex::new(HashMap::new()));
    }
    Arc::new(db)
}
```

Then, finding the cell for any given key becomes a two step process. First, the key
is used to identify which shard it is part of. Then, the key is looked up in the
`HashMap`.

```rust
let shard = db[hash(key) % db.len()].lock().unwrap();
shard.insert(key, value);
```

The simple implementation outlined above requires using a fixed number of shards,
and the number of shards cannot be changed once the sharded map is created.
