// REPL module -- will be implemented in Task 2
use crate::identity::{Identity, IdentityStore};
use anyhow::Result;
use std::net::SocketAddr;

/// Run the terminal REPL for human input.
pub async fn run(
    _server_addr: SocketAddr,
    _identity: Identity,
    _identity_store: IdentityStore,
) -> Result<()> {
    // Stub -- implemented in Task 2
    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    Ok(())
}
