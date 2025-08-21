use mini_redis::{Result, client};

#[tokio::main]
async fn main() -> Result<()> {
    // Asynchronously establish a TCP connection with the specified remote address.
    // Once the connection is established a `client` handle is returned.
    let mut client = client::connect("127.0.0.1:6379").await?;

    client.set("hello", "world".into()).await?;

    let result = client.get("hello").await?;
    println!("got value from the server: result={:?}", result);

    Ok(())
}
