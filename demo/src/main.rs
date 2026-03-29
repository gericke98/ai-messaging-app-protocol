mod agent;
mod federation;
mod identity;
mod protocol;
mod relay;
mod render;
mod repl;
mod server;
mod signing;
mod streaming;
mod tls;

use anyhow::Result;
use clap::Parser;
use std::sync::Arc;

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

    let mode = cli.mode.clone();
    match mode.as_deref() {
        None => run_legacy_demo().await,
        Some(mode @ ("human" | "agent")) => run_federation(cli, mode.to_string()).await,
        Some(unknown) => {
            eprintln!("Unknown mode: '{}'. Valid modes: human, agent", unknown);
            std::process::exit(1);
        }
    }
}

/// Run the federation mode with two-listener architecture.
///
/// - Listener 1: Plain TCP for local WebSocket connections (REPL / agent)
/// - Listener 2: mTLS TCP for federation relay with cert fingerprint verification (FAUT-02)
async fn run_federation(cli: Cli, mode: String) -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .init();

    // 1. Validate required flags
    let server_id = cli.identity.as_deref().unwrap_or_else(|| {
        eprintln!("Error: --identity is required in federation mode");
        std::process::exit(1);
    });
    let cert_path = cli.cert.as_deref().unwrap_or_else(|| {
        eprintln!("Error: --cert is required in federation mode");
        std::process::exit(1);
    });
    let key_path = cli.key.as_deref().unwrap_or_else(|| {
        eprintln!("Error: --key is required in federation mode");
        std::process::exit(1);
    });
    let ca_cert_path = cli.ca_cert.as_deref().unwrap_or_else(|| {
        eprintln!("Error: --ca-cert is required in federation mode");
        std::process::exit(1);
    });

    // 2. Generate server Ed25519 keypair
    let server_keypair = Arc::new(identity::generate_keypair(&format!(
        "server@{}",
        server_id
    ))?);

    // 3. Load TLS cert and compute fingerprint
    let cert_der = tls::load_cert_der(cert_path);
    let tls_fingerprint = tls::compute_cert_fingerprint(&cert_der);

    // 4. Build ServerIdentity
    let server_identity = Arc::new(identity::build_server_identity(
        server_id,
        &server_keypair,
        cli.federation_port,
        &tls_fingerprint,
    ));

    // 5. Build mTLS HTTP client and RelaySender
    let http_client = tls::build_mtls_client(ca_cert_path, cert_path, key_path);
    let remote_cache = identity::RemoteServerCache::new();
    let relay_sender = Arc::new(relay::RelaySender::new(
        http_client.clone(),
        server_keypair.clone(),
        server_id.to_string(),
        remote_cache.clone(),
    ));

    // 6. Build identity store and register local participant
    let identity_store = identity::IdentityStore::new();
    let local_identity;
    let agent_card;

    match mode.as_str() {
        "human" => {
            let human_addr = format!("alice@{}", server_id);
            local_identity = identity::generate_keypair(&human_addr)?;
            identity_store.register(
                &local_identity.address,
                &local_identity.public_key_b64(),
                "human",
                server_id,
                None,
            );
            agent_card = serde_json::json!(null);
        }
        "agent" => {
            let agent_addr = format!("helper@{}", server_id);
            local_identity = identity::generate_keypair(&agent_addr)?;
            identity_store.register(
                &local_identity.address,
                &local_identity.public_key_b64(),
                "agent",
                server_id,
                Some(format!(
                    "https://{}:{}/.well-known/ainm/helper/agent-card.json",
                    server_id, cli.federation_port
                )),
            );
            agent_card = serde_json::json!({
                "name": "AINM Federation Helper",
                "description": "A demo agent for federated messaging",
                "url": format!("https://{}", server_id),
                "version": "0.1.0",
                "protocolVersion": "0.1.0",
                "capabilities": { "streaming": true, "pushNotifications": false },
                "skills": [{ "id": "echo", "name": "Echo", "description": "Echoes messages back" }],
                "ainm": {
                    "protocolVersion": "0.1.0",
                    "address": local_identity.address,
                    "type": "agent",
                    "contentTypes": ["text"],
                    "streaming": { "supported": true, "parallelBlocks": false, "preferredChunkSize": "token" }
                }
            });
        }
        _ => unreachable!(),
    }

    // 7. Resolve remote server identity and pre-seed participant keys
    if let Some(ref remote) = cli.remote {
        match resolve_remote_identity(&http_client, remote).await {
            Ok(remote_identity) => {
                tracing::info!(
                    "[federation] Resolved remote server: {}",
                    remote_identity.server_id
                );
                let remote_domain = remote_identity.server_id.clone();
                remote_cache.insert(remote_identity);

                // Pre-seed remote participant identity (DEMO-02)
                // Since generate_keypair is non-deterministic, we need to exchange
                // public keys. For the demo, we fetch the remote server's participant
                // key via a well-known endpoint. If that fails, we log a warning --
                // the relay handler will reject unknown senders until keys are exchanged.
                match mode.as_str() {
                    "human" => {
                        // We need helper@{remote_domain}'s public key
                        let remote_participant = format!("helper@{}", remote_domain);
                        match fetch_participant_key(&http_client, remote, &remote_participant).await
                        {
                            Ok((addr, pk_b64)) => {
                                identity_store.register(
                                    &addr,
                                    &pk_b64,
                                    "agent",
                                    &remote_domain,
                                    None,
                                );
                                tracing::info!(
                                    "[federation] Pre-seeded remote participant: {}",
                                    addr
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "[federation] Could not pre-seed {}: {}. Will be registered on first relay.",
                                    remote_participant,
                                    e
                                );
                            }
                        }
                    }
                    "agent" => {
                        // We need alice@{remote_domain}'s public key
                        let remote_participant = format!("alice@{}", remote_domain);
                        match fetch_participant_key(&http_client, remote, &remote_participant).await
                        {
                            Ok((addr, pk_b64)) => {
                                identity_store.register(
                                    &addr,
                                    &pk_b64,
                                    "human",
                                    &remote_domain,
                                    None,
                                );
                                tracing::info!(
                                    "[federation] Pre-seeded remote participant: {}",
                                    addr
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "[federation] Could not pre-seed {}: {}. Will be registered on first relay.",
                                    remote_participant,
                                    e
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
            Err(e) => {
                tracing::warn!(
                    "[federation] Could not resolve remote server {}: {}. Will retry on first relay.",
                    remote,
                    e
                );
            }
        }
    }

    // 8. Create broadcast channel for local delivery
    let (local_delivery_tx, _) = tokio::sync::broadcast::channel::<String>(256);

    // 9. Build FederationState
    let thread_registry = federation::ThreadRegistry::new();
    let fed_state = federation::FederationState {
        server_id: server_id.to_string(),
        server_keypair: server_keypair.clone(),
        server_identity: server_identity.clone(),
        thread_registry: thread_registry.clone(),
        identity_store: identity_store.clone(),
        remote_cache: remote_cache.clone(),
        relay_sender: Some(relay_sender.clone()),
        local_delivery_tx: local_delivery_tx.clone(),
    };

    // 10. Start Listener 1: Plain TCP (local WebSocket)
    let fed_context = server::FederationContext {
        server_id: server_id.to_string(),
        thread_registry: thread_registry.clone(),
        relay_sender: relay_sender.clone(),
        local_delivery_tx: local_delivery_tx.clone(),
        remote_cache: remote_cache.clone(),
    };

    let (server_addr, server_handle) = server::start_federation(
        identity_store.clone(),
        agent_card.clone(),
        Some(fed_context),
        cli.port,
    )
    .await?;

    // 11. Start Listener 2: mTLS TCP (federation relay) with FAUT-02 cert fingerprint verification
    let mtls_config = tls::build_mtls_server_config(ca_cert_path, cert_path, key_path);

    let fed_app = axum::Router::new()
        .route(
            "/.well-known/ainm/server",
            axum::routing::get(identity::server_identity_handler),
        )
        .with_state(server_identity.clone())
        .route(
            "/ainm/v1/relay",
            axum::routing::post(federation::relay_handler),
        )
        .with_state(fed_state.clone())
        .route(
            "/.well-known/ainm/participant/:address",
            axum::routing::get(participant_key_handler),
        )
        .with_state(identity_store.clone());

    let tls_acceptor = tokio_rustls::TlsAcceptor::from(mtls_config);
    let tls_listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", cli.federation_port))
        .await?;
    let fed_addr = tls_listener.local_addr()?;

    let remote_cache_for_tls = remote_cache.clone();
    let fed_handle = tokio::spawn(async move {
        loop {
            match tls_listener.accept().await {
                Ok((stream, addr)) => {
                    let acceptor = tls_acceptor.clone();
                    let app = fed_app.clone();
                    let cache = remote_cache_for_tls.clone();
                    tokio::spawn(async move {
                        match acceptor.accept(stream).await {
                            Ok(tls_stream) => {
                                // --- FAUT-02 Layer 1: TLS cert fingerprint verification ---
                                let (_, server_conn) = tls_stream.get_ref();
                                let peer_certs = server_conn.peer_certificates();

                                if let Some(certs) = peer_certs {
                                    if let Some(first_cert) = certs.first() {
                                        let peer_fingerprint =
                                            tls::compute_cert_fingerprint(first_cert.as_ref());

                                        let fingerprint_valid =
                                            cache.find_by_fingerprint(&peer_fingerprint).is_some();

                                        if !fingerprint_valid {
                                            tracing::error!(
                                                "[federation] FAUT-02 layer 1 REJECTED: peer cert fingerprint {} from {} not in remote cache",
                                                peer_fingerprint,
                                                addr
                                            );
                                            return;
                                        }
                                        tracing::info!(
                                            "[federation] FAUT-02 layer 1 OK: peer cert fingerprint {} from {}",
                                            peer_fingerprint,
                                            addr
                                        );
                                    } else {
                                        tracing::error!(
                                            "[federation] No peer certificate presented from {}",
                                            addr
                                        );
                                        return;
                                    }
                                } else {
                                    tracing::error!(
                                        "[federation] No peer certificates from {}",
                                        addr
                                    );
                                    return;
                                }
                                // --- End FAUT-02 Layer 1 ---

                                // Serve the axum router over the verified TLS stream
                                let io = hyper_util::rt::TokioIo::new(tls_stream);
                                let tower_service = app.into_service();
                                let hyper_service =
                                    hyper_util::service::TowerToHyperService::new(tower_service);
                                if let Err(e) = hyper_util::server::conn::auto::Builder::new(
                                    hyper_util::rt::TokioExecutor::new(),
                                )
                                .serve_connection(io, hyper_service)
                                .await
                                {
                                    tracing::error!(
                                        "[federation] HTTP error from {}: {}",
                                        addr,
                                        e
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::error!("TLS handshake failed from {}: {}", addr, e);
                            }
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("[federation] Accept error: {}", e);
                }
            }
        }
    });

    // 12. Start relay send loop
    let _relay_handle = relay_sender.start_send_loop();

    // 13. Print federation banner
    println!();
    println!("========================================");
    println!("  AINM Federation Demo v0.1.0");
    println!("========================================");
    println!("  Mode:       {}", mode);
    println!("  Identity:   {}", server_id);
    println!("  Local:      ws://0.0.0.0:{}/ws", server_addr.port());
    println!(
        "  Federation: https://0.0.0.0:{}/ainm/v1/relay",
        fed_addr.port()
    );
    println!(
        "  Remote:     {}",
        cli.remote.as_deref().unwrap_or("none")
    );
    println!("========================================");
    println!();

    // 14. Mode branching
    match mode.as_str() {
        "human" => {
            // Run REPL connecting to local WebSocket server
            let repl_result =
                repl::run(server_addr, local_identity, identity_store.clone()).await;
            server_handle.abort();
            fed_handle.abort();
            if let Err(e) = repl_result {
                tracing::error!("REPL error: {}", e);
            }
            println!("\nGoodbye!");
        }
        "agent" => {
            // Run headless agent: subscribe to local_delivery_tx and process messages
            tracing::info!(
                "[agent] Running headless agent as {}",
                local_identity.address
            );
            let mut delivery_rx = local_delivery_tx.subscribe();
            loop {
                match delivery_rx.recv().await {
                    Ok(msg_text) => {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&msg_text) {
                            let method = parsed
                                .get("method")
                                .and_then(|m| m.as_str())
                                .unwrap_or("");
                            if method == "ainm.message.deliver" {
                                if let Some(message) =
                                    parsed.get("params").and_then(|p| p.get("message"))
                                {
                                    let from = message
                                        .get("from")
                                        .and_then(|f| f.as_str())
                                        .unwrap_or("unknown");
                                    let thread_id = message
                                        .get("thread_id")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("");
                                    let text = message
                                        .get("content")
                                        .and_then(|c| c.as_array())
                                        .and_then(|blocks| {
                                            blocks.iter().find_map(|b| {
                                                if b.get("type").and_then(|t| t.as_str())
                                                    == Some("text")
                                                {
                                                    b.get("text")
                                                        .and_then(|t| t.as_str())
                                                        .map(|s| s.to_string())
                                                } else {
                                                    None
                                                }
                                            })
                                        })
                                        .unwrap_or_default();

                                    tracing::info!(
                                        "[agent] Received from {}: \"{}\"",
                                        from,
                                        text
                                    );

                                    // Generate response
                                    let response_text = format!(
                                        "Echo from {}: You said \"{}\"",
                                        local_identity.address, text
                                    );

                                    let mut response_envelope = serde_json::json!({
                                        "id": protocol::new_message_id(),
                                        "from": local_identity.address,
                                        "thread_id": thread_id,
                                        "timestamp": protocol::now_rfc3339(),
                                        "type": "message",
                                        "content": [{
                                            "type": "text",
                                            "version": "1.0",
                                            "source": "agent",
                                            "text": response_text,
                                            "fallback": response_text
                                        }]
                                    });

                                    if let Err(e) = signing::sign_envelope(
                                        &local_identity.key_pair,
                                        &mut response_envelope,
                                    ) {
                                        tracing::error!("[agent] Signing error: {}", e);
                                        continue;
                                    }

                                    // Determine where to relay the response
                                    let origin =
                                        federation::parse_origin_domain(thread_id).unwrap_or("");

                                    if origin != server_id {
                                        // Thread originated on remote server -- relay back
                                        relay_sender.enqueue(origin, response_envelope.clone());
                                        tracing::info!(
                                            "[agent] Response relayed to origin: {}",
                                            origin
                                        );
                                    } else {
                                        // Local thread -- deliver locally
                                        let deliver = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "method": "ainm.message.deliver",
                                            "params": {
                                                "message": response_envelope,
                                                "verified": true,
                                            }
                                        });
                                        let _ = local_delivery_tx
                                            .send(serde_json::to_string(&deliver).unwrap());
                                    }
                                }
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("[agent] Delivery channel lagged by {} messages", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        tracing::info!("[agent] Delivery channel closed, shutting down");
                        break;
                    }
                }
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}

/// Resolve a remote server's federation identity via GET /.well-known/ainm/server.
async fn resolve_remote_identity(
    client: &reqwest::Client,
    remote: &str,
) -> Result<identity::ServerIdentity> {
    let url = format!("https://{}/.well-known/ainm/server", remote);
    let resp = client.get(&url).send().await?;
    let body: serde_json::Value = resp.json().await?;

    Ok(identity::ServerIdentity {
        server_id: body
            .get("server_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        public_key_b64: body
            .get("public_key")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        relay_endpoint: body
            .get("relay_endpoint")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        tls_fingerprint: body
            .get("tls_fingerprint")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        supported_versions: body
            .get("supported_versions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// Fetch a participant's public key from a remote server.
///
/// Used for pre-seeding identity store so relay_handler accepts messages
/// from known senders (DEMO-02).
async fn fetch_participant_key(
    client: &reqwest::Client,
    remote: &str,
    address: &str,
) -> Result<(String, String)> {
    let url = format!(
        "https://{}/.well-known/ainm/participant/{}",
        remote, address
    );
    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }
    let body: serde_json::Value = resp.json().await?;
    let addr = body
        .get("address")
        .and_then(|v| v.as_str())
        .unwrap_or(address)
        .to_string();
    let pk = body
        .get("public_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing public_key in response"))?
        .to_string();
    Ok((addr, pk))
}

/// Axum handler: GET /.well-known/ainm/participant/:address
///
/// Returns a participant's public key from the local identity store.
/// Used for cross-server key exchange during federation setup.
async fn participant_key_handler(
    axum::extract::Path(address): axum::extract::Path<String>,
    axum::extract::State(store): axum::extract::State<identity::IdentityStore>,
) -> impl axum::response::IntoResponse {
    if let Some(pk_bytes) = store.get_public_key_bytes(&address) {
        let pk_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            &pk_bytes,
        );
        (
            axum::http::StatusCode::OK,
            axum::Json(serde_json::json!({
                "address": address,
                "public_key": pk_b64,
            })),
        )
    } else {
        (
            axum::http::StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({
                "error": "participant not found"
            })),
        )
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

    // 4. Start Axum server (legacy mode -- no federation)
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
