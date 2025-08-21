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


