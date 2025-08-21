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

*In computer programming, a green thread is a thread that is scheduled by a runtime library or virtual machine instead of natively by the underlying operating system. Green threads emulate multithreaded environments without relying on any native OS abilities, and they are managed in user space instead of kernel space, enabling them to work in environments that do not have native thread support.*

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

