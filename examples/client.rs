use rust_http::message::{Method::Get, RequestBuilder};
use rust_http::{self, client, client::error::ClientError};

#[tokio::main]
async fn main() -> Result<(), ClientError> {
    let mut req = RequestBuilder::new(Get, "/")
        .header("Connection", "keep-alive")
        .build();

    let resp = client::send_request("google.com", &mut req).await?;

    println!("Got response: {resp:?}");

    Ok(())
}
