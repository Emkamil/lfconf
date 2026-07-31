mod interface;

use crate::interface::LfConfInterface;
use lfconf::Store;
use std::sync::Arc;
use zbus::connection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Arc::new(Store::new());
    let interface = LfConfInterface { store };

    let _conn = connection::Builder::session()?
        .name("org.lfbe.lfconf")?
        .serve_at("/org/lfbe/lfconf", interface)?
        .build()
        .await?;

    println!("lfconfd: Active.");
    std::future::pending::<()>().await;
    Ok(())
}