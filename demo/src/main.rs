mod agent;
mod identity;
mod protocol;
mod render;
mod repl;
mod server;
mod signing;
mod streaming;

use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(name = "ainm-demo", about = "AINM Protocol Demo")]
struct Cli {
    /// Server mode: human (REPL) or agent (headless)
    #[arg(long)]
    mode: Option<String>,

    /// Local WebSocket port
    #[arg(long, default_value = "8080")]
    port: u16,

    /// Federation relay port (mTLS)
    #[arg(long, default_value = "8443")]
    federation_port: u16,

    /// Server identity domain (e.g., server-a.local)
    #[arg(long)]
    identity: Option<String>,

    /// Path to TLS certificate PEM
    #[arg(long)]
    cert: Option<String>,

    /// Path to TLS private key PEM
    #[arg(long)]
    key: Option<String>,

    /// Path to CA certificate PEM for mTLS verification
    #[arg(long)]
    ca_cert: Option<String>,

    /// Remote server to federate with (e.g., server-b.local:8443)
    #[arg(long)]
    remote: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.mode.as_deref() {
        None => run_legacy_demo().await,
        Some(mode @ ("human" | "agent")) => {
            println!(
                "Federation mode: {} (not yet implemented, see plans 02-04)",
                mode
            );
            Ok(())
        }
        Some(unknown) => {
            eprintln!("Unknown mode: '{}'. Valid modes: human, agent", unknown);
            std::process::exit(1);
        }
    }
}

async fn run_legacy_demo() -> Result<()> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .init();

    // 1. Generate Ed25519 key pairs for human and agent
    let human_identity = identity::generate_keypair("alice@demo.ainm.local")?;
    let agent_identity = identity::generate_keypair("helper@demo.ainm.local")?;

    // 2. Build identity store and register both participants
    let identity_store = identity::IdentityStore::new();

    // Register human
    identity_store.register(
        &human_identity.address,
        &human_identity.public_key_b64(),
        "human",
        "ws://127.0.0.1:0/ws",
        None,
    );

    // Register agent with Agent Card URL
    identity_store.register(
        &agent_identity.address,
        &agent_identity.public_key_b64(),
        "agent",
        "ws://127.0.0.1:0/ws",
        Some("http://127.0.0.1:0/.well-known/ainm/helper/agent-card.json".to_string()),
    );

    // 3. Build Agent Card for helper@demo.ainm.local
    let agent_card = serde_json::json!({
        "name": "AINM Demo Helper",
        "description": "A demo agent showcasing the AI-Native Messaging Protocol",
        "url": "https://demo.ainm.local",
        "version": "0.1.0",
        "protocolVersion": "0.1.0",
        "capabilities": {
            "streaming": true,
            "pushNotifications": false
        },
        "skills": [
            {
                "id": "demo",
                "name": "Protocol Demo",
                "description": "Demonstrates streaming, forms, buttons, approvals, and tool calls"
            }
        ],
        "ainm": {
            "protocolVersion": "0.1.0",
            "address": "helper@demo.ainm.local",
            "type": "agent",
            "contentTypes": ["text", "code", "button", "form", "approval"],
            "streaming": {
                "supported": true,
                "parallelBlocks": false,
                "preferredChunkSize": "token"
            },
            "threading": {
                "subThreads": false,
                "maxConcurrentThreads": 1
            },
            "memory": {
                "archivalSupported": true,
                "contextFramePreferred": true
            },
            "tools": [
                {
                    "name": "weather_lookup",
                    "description": "Look up current weather data for a city",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "city": { "type": "string" }
                        },
                        "required": ["city"]
                    }
                }
            ]
        }
    });

    // 4. Start Axum server
    let (server_addr, server_handle) =
        server::start(identity_store.clone(), agent_card).await?;

    // Update identity store with actual server address
    identity_store.register(
        &human_identity.address,
        &human_identity.public_key_b64(),
        "human",
        &format!("ws://{}/ws", server_addr),
        None,
    );
    identity_store.register(
        &agent_identity.address,
        &agent_identity.public_key_b64(),
        "agent",
        &format!("ws://{}/ws", server_addr),
        Some(format!(
            "http://{}/.well-known/ainm/helper/agent-card.json",
            server_addr
        )),
    );

    // 5. Print welcome banner
    println!();
    println!("========================================");
    println!("  AINM Protocol Demo v0.1.0");
    println!("========================================");
    println!();
    println!("  Server:  ws://{}/ws", server_addr);
    println!("  Human:   {}", human_identity.address);
    println!("  Agent:   {}", agent_identity.address);
    println!();
    println!("  Commands:");
    println!("    hello    - Streaming text response");
    println!("    form     - FormBlock demo");
    println!("    approve  - ApprovalBlock demo");
    println!("    tool     - Tool visibility demo");
    println!("    button   - ButtonBlock demo");
    println!("    /card    - View Agent Card");
    println!("    /memory  - Memory store, query, context frame demo");
    println!("    /bridge  - MCP and A2A bridge compatibility demo");
    println!("    /help    - Show commands");
    println!("    /quit    - Exit");
    println!();
    println!("  Every message is Ed25519-signed and verified.");
    println!("========================================");
    println!();

    // 6. Spawn agent task
    let agent_handle = tokio::spawn(agent::run(
        server_addr,
        agent_identity,
        identity_store.clone(),
    ));

    // Give agent a moment to connect
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // 7. Run REPL (this blocks until the user quits)
    let repl_result = repl::run(server_addr, human_identity, identity_store).await;

    // 8. Shutdown
    server_handle.abort();
    agent_handle.abort();

    if let Err(e) = repl_result {
        tracing::error!("REPL error: {}", e);
    }

    println!("\nGoodbye!");
    Ok(())
}
