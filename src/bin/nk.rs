#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    neatkube::run().await
}
