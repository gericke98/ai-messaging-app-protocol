mod accounts;
mod agent;
mod auth_token;
mod briefing;
mod categories;
mod compiler;
mod config;
mod db;
mod email;
mod error;
mod escrow;
mod federation;
mod graph;
mod hosted_agent;
mod llm;
mod identity;
mod mcp;
mod persistence;
mod protocol;
mod relay;
mod render;
mod repl;
mod repository;
mod retrieval;
mod runtime;
mod runtime_server;
mod server;
mod signing;
mod streaming;
mod tools;
mod tools_memory;
mod tls;

use anyhow::Result;
use axum::response::IntoResponse;
use base64::Engine;
use clap::Parser;
use std::sync::Arc;

/// (hosted agent, its LLM client, curated MCP tool providers) built at startup.
type HostedAgentBuild = (
    Option<std::sync::Arc<hosted_agent::HostedAgent>>,
    Option<std::sync::Arc<dyn llm::LlmClient>>,
    std::sync::Arc<Vec<std::sync::Arc<dyn tools::ToolProvider>>>,
);

#[derive(Parser)]
#[command(name = "ainm-server", about = "AINM Protocol Server")]
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

    /// Path to persistent data directory for redb
    #[arg(long, default_value = "./data")]
    data_dir: String,

    /// Additional TLS fingerprint to include in .well-known during cert rotation.
    /// When rotating certificates, pass the OLD certificate's fingerprint here
    /// so that tls_fingerprints serves [new, old] enabling TOFU auto-update.
    #[arg(long)]
    additional_fingerprint: Option<String>,

    /// Run in UAT automation mode: ephemeral SQLite, fixed port 8080, INFO logs to tmp/uat-server.log.
    /// Used by `make uat` and the AINMMessengerUITests scheme; not for production (Phase 14-02).
    #[arg(long)]
    uat: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Phase 14-02: --uat overrides all other mode selection. Hermetic automation harness
    // expects a deterministic boot path regardless of any other flags set on the command line.
    if cli.uat {
        return run_uat(cli).await;
    }

    let mode = cli.mode.clone();
    match mode.as_deref() {
        None => run_reference_scenario().await,
        Some(mode @ ("human" | "agent")) => run_federation(cli, mode.to_string()).await,
        Some(unknown) => {
            eprintln!("Unknown mode: '{}'. Valid modes: human, agent", unknown);
            std::process::exit(1);
        }
    }
}

/// Phase 14-02 UAT automation harness entry point.
///
/// Boots an AINM server on `127.0.0.1:8080` (legacy WebSocket path, no federation), registers
/// a `/health` endpoint for `make uat` readiness polling, writes INFO-level logs to
/// `tmp/uat-server.log` relative to the repo root, uses a fresh system tempdir for ephemeral
/// storage, and provisions the deterministic `uat-user@localhost` + `helper@localhost` identities
/// that iOS test fixtures expect. Blocks on Ctrl-C; Makefile sends SIGTERM via `trap EXIT INT TERM`.
async fn run_uat(_cli: Cli) -> Result<()> {
    use tracing::Level;

    // 1. Logging: INFO level, file writer at tmp/uat-server.log.
    //    CARGO_MANIFEST_DIR = server/; parent = repo root; log path = tmp/uat-server.log.
    //    Resolving relative to CARGO_MANIFEST_DIR normalizes both `cd server && cargo run` and
    //    `cargo run --manifest-path server/Cargo.toml` invocation shapes.
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR has parent");
    let log_dir = repo_root.join("tmp");
    std::fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join("uat-server.log");
    let log_file = std::fs::File::create(&log_path)?;
    let _ = tracing_subscriber::fmt()
        .with_target(false)
        .with_max_level(Level::INFO)
        .with_writer(std::sync::Mutex::new(log_file))
        .try_init();

    // 2. Ephemeral tempdir — automatic teardown when the guard drops at process exit.
    //    legacy/start_with_port path uses in-memory ServerState; no DB wiring needed today,
    //    but surfacing the tempdir lets 14-03+ layer in GRDB-backed archival seed data cheaply.
    let tmp_dir = tempfile::tempdir()?;
    tracing::info!("[uat] ephemeral db dir: {}", tmp_dir.path().display());

    // 3. Provision deterministic identities that match the iOS UAT fixtures (D-04).
    let human = identity::generate_keypair("uat-user@localhost")?;
    let agent = identity::generate_keypair("helper@localhost")?;
    let identity_store = identity::IdentityStore::new();
    identity_store.register(
        &human.address,
        &human.public_key_b64(),
        "human",
        "localhost",
        None,
    );
    identity_store.register(
        &agent.address,
        &agent.public_key_b64(),
        "agent",
        "localhost",
        Some("http://localhost:8080/.well-known/ainm/helper/agent-card.json".to_string()),
    );

    // 4. Build the minimal agent card the AINMClient handshake expects.
    let agent_card = build_uat_agent_card();

    // 5. Boot server on fixed port 8080 with /health registered.
    let (addr, handle) =
        server::start_with_port(identity_store.clone(), agent_card, 8080).await?;
    tracing::info!(
        "[uat] server ready on ws://{}/ws, GET /health returns 200",
        addr
    );
    println!("[uat] server ready on ws://{}/ws", addr);

    // 6. Block on Ctrl-C. Makefile teardown fires SIGTERM/SIGINT via trap EXIT INT TERM.
    tokio::signal::ctrl_c().await?;
    tracing::info!("[uat] received ctrl-c; shutting down");
    handle.abort();

    Ok(())
}

/// Minimal agent card for UAT mode — sufficient for AINMClient handshake and scenario fixtures.
fn build_uat_agent_card() -> serde_json::Value {
    serde_json::json!({
        "name": "helper",
        "address": "helper@localhost",
        "description": "UAT helper agent (Phase 14-02)",
        "url": "http://localhost:8080",
        "version": "0.1.0",
        "protocolVersion": "0.1.0",
        "capabilities": {
            "streaming": true,
            "pushNotifications": false
        },
        "skills": [],
        "ainm": {
            "protocolVersion": "0.1.0",
            "address": "helper@localhost",
            "type": "agent",
            "contentTypes": ["text", "code", "button", "form", "approval"],
            "streaming": {
                "supported": true,
                "parallelBlocks": false,
                "preferredChunkSize": "token"
            },
            "threading": {
                "subThreads": true,
                "maxConcurrentThreads": 1
            },
            "memory": {
                "archivalSupported": true,
                "contextFramePreferred": true
            },
            "tools": []
        }
    })
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
    let mut tls_fingerprints = vec![tls_fingerprint.clone()];
    if let Some(ref extra_fp) = cli.additional_fingerprint {
        tls_fingerprints.push(extra_fp.clone());
        tracing::info!("[identity] Cert rotation mode: serving tls_fingerprints = {:?}", tls_fingerprints);
    }

    // 4. Build ServerIdentity
    let server_identity = Arc::new(identity::build_server_identity(
        server_id,
        &server_keypair,
        cli.federation_port,
        tls_fingerprints,
    ));

    // 4b. Initialize persistent storage (redb)
    let data_dir = &cli.data_dir;
    std::fs::create_dir_all(data_dir).expect("failed to create data directory");
    let persistence = persistence::PersistenceLayer::open(
        &format!("{}/ainm.redb", data_dir),
    )
    .expect("failed to open redb database");

    // 5. Build mTLS HTTP client and RelaySender
    let http_client = tls::build_mtls_client(ca_cert_path, cert_path, key_path);
    let remote_cache = identity::RemoteServerCache::new();
    let relay_sender = Arc::new(relay::RelaySender::new(
        http_client.clone(),
        server_keypair.clone(),
        server_id.to_string(),
        remote_cache.clone(),
        persistence.clone(),
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
                "description": "A reference agent for federated messaging",
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
                // public keys. Here we fetch the remote server's participant
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

    // 7b. Initialize PostgreSQL if configured
    let app_config = config::load().unwrap_or_else(|e| {
        tracing::warn!("[config] Failed to load config: {e}, using defaults");
        config::AppConfig { database: config::DatabaseConfig::default(), mcp_servers: vec![] }
    });
    let db_pool = if app_config.database.url.is_some() {
        match db::init_pool(&app_config.database).await {
            Ok(pool) => {
                tracing::info!("[db] PostgreSQL connected and migrations applied");
                Some(pool)
            }
            Err(e) => {
                tracing::warn!("[db] PostgreSQL init failed: {e}. Running without database persistence.");
                None
            }
        }
    } else {
        tracing::info!("[db] No AINM_DATABASE__URL configured, using in-memory storage");
        None
    };

    // 7c. Initialize LLM client and compilation worker (if DB is available)
    let (compile_tx, compile_notify_tx): (
        Option<tokio::sync::mpsc::Sender<compiler::CompileRequest>>,
        Option<tokio::sync::broadcast::Sender<String>>,
    ) = if let Some(ref pool) = db_pool {
        let llm_api_key = std::env::var("AINM_LLM__API_KEY").ok();
        let llm_model = std::env::var("AINM_LLM__MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
        let llm_base_url = std::env::var("AINM_LLM__BASE_URL").ok();

        let llm_client: Arc<dyn llm::LlmClient> = if let Some(api_key) = llm_api_key {
            tracing::info!("[llm] Using OpenAI-compatible client (model: {llm_model})");
            Arc::new(llm::OpenAiClient::new(api_key, llm_model, llm_base_url))
        } else {
            tracing::info!("[llm] No AINM_LLM__API_KEY configured, using mock LLM client");
            Arc::new(llm::MockLlmClient::empty())
        };

        let (tx, rx) = tokio::sync::mpsc::channel::<compiler::CompileRequest>(64);
        let (notify_tx, _) = tokio::sync::broadcast::channel::<String>(64);
        compiler::spawn_compilation_worker(rx, pool.clone(), llm_client, notify_tx.clone());
        (Some(tx), Some(notify_tx))
    } else {
        (None, None)
    };

    // 7d. Account identity: token signer + email sender
    let token_secret = std::env::var("AINM_TOKEN_SECRET")
        .map(|s| s.into_bytes())
        .unwrap_or_else(|_| {
            use rand::RngCore;
            let mut b = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut b);
            tracing::warn!("[accounts] AINM_TOKEN_SECRET unset; using a random per-process secret (tokens won't survive restart)");
            b.to_vec()
        });
    let token_signer = auth_token::TokenSigner::new(&token_secret);

    let email_sender: std::sync::Arc<dyn email::EmailSender> =
        match (std::env::var("AINM_EMAIL__RESEND_API_KEY").ok(), std::env::var("AINM_EMAIL__FROM").ok()) {
            (Some(key), Some(from)) => {
                tracing::info!("[email] using Resend sender");
                std::sync::Arc::new(email::ResendEmailSender::new(key, from))
            }
            _ => {
                tracing::info!("[email] no AINM_EMAIL__RESEND_API_KEY; using mock email sender (codes logged)");
                std::sync::Arc::new(email::MockEmailSender::new())
            }
        };

    // 7e. Provision the hosted agent + its LLM client + curated MCP providers (B-8).
    //     Requires a DB (the agent signer + prompt live in hosted_agents). The LLM client
    //     is independent of the compilation worker's (different lifecycle / sharing).
    let (hosted_agent, hosted_llm, mcp_providers): HostedAgentBuild = if let Some(ref pool) = db_pool {
        let agent = hosted_agent::provision(pool, server_id).await
            .map(std::sync::Arc::new)
            .map_err(|e| tracing::warn!("[hosted-agent] provisioning failed: {e}"))
            .ok();
        let llm: Option<std::sync::Arc<dyn llm::LlmClient>> = match std::env::var("AINM_LLM__API_KEY") {
            Ok(key) => Some(std::sync::Arc::new(llm::OpenAiClient::new(
                key,
                std::env::var("AINM_LLM__MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string()),
                std::env::var("AINM_LLM__BASE_URL").ok(),
            ))),
            Err(_) => None,
        };
        let providers = mcp::build_providers_from_config(&app_config).await;
        (agent, llm, std::sync::Arc::new(providers))
    } else {
        (None, None, std::sync::Arc::new(vec![]))
    };

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
        persistence: persistence.clone(),
        http_client: Some(http_client.clone()),
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
        db_pool,
        compile_tx,
        compile_notify_tx,
        email_sender,
        token_signer,
        server_id.to_string(),
        hosted_agent,
        hosted_llm,
        mcp_providers,
    )
    .await?;

    // 11. Start Listener 2: mTLS TCP (federation relay) with FAUT-02 cert fingerprint verification
    let mtls_config = tls::build_mtls_server_config(ca_cert_path, cert_path, key_path);

    // Build agent card handler for federation listener (serves same card as local listener)
    let agent_card_for_fed = Arc::new(agent_card.clone());
    let fed_agent_card_handler = {
        let card = agent_card_for_fed.clone();
        move |headers: axum::http::HeaderMap, axum::extract::Path(_agent_name): axum::extract::Path<String>| {
            let card = card.clone();
            async move {
                let body = serde_json::to_vec(&*card).unwrap();
                let hash = ring::digest::digest(&ring::digest::SHA256, &body);
                let etag_value = format!(
                    "\"{}\"",
                    base64::engine::general_purpose::URL_SAFE_NO_PAD
                        .encode(hash.as_ref())
                );
                if let Some(if_none_match) = headers.get(axum::http::header::IF_NONE_MATCH) {
                    if if_none_match.to_str().unwrap_or("") == etag_value {
                        return (
                            axum::http::StatusCode::NOT_MODIFIED,
                            [(axum::http::header::ETAG, etag_value)],
                        )
                            .into_response();
                    }
                }
                (
                    axum::http::StatusCode::OK,
                    [
                        (
                            axum::http::header::CONTENT_TYPE,
                            "application/json".to_string(),
                        ),
                        (axum::http::header::ETAG, etag_value),
                    ],
                    body,
                )
                    .into_response()
            }
        }
    };

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
            "/.well-known/ainm/participant/{address}",
            axum::routing::get(participant_key_handler),
        )
        .with_state(identity_store.clone())
        .route(
            "/.well-known/ainm/{agent_name}/agent-card.json",
            axum::routing::get(fed_agent_card_handler),
        );

    let tls_acceptor = tokio_rustls::TlsAcceptor::from(mtls_config);
    let tls_listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", cli.federation_port))
        .await?;
    let fed_addr = tls_listener.local_addr()?;

    let remote_cache_for_tls = remote_cache.clone();
    let persistence_for_tls = persistence.clone();
    let fed_handle = tokio::spawn(async move {
        loop {
            match tls_listener.accept().await {
                Ok((stream, addr)) => {
                    let acceptor = tls_acceptor.clone();
                    let app = fed_app.clone();
                    let cache = remote_cache_for_tls.clone();
                    let persistence_conn = persistence_for_tls.clone();
                    tokio::spawn(async move {
                        match acceptor.accept(stream).await {
                            Ok(tls_stream) => {
                                // --- FAUT-02 Layer 1 + FAUT-04 TOFU verification ---
                                // Note: fingerprint check is permissive at TLS layer to allow
                                // identity bootstrapping (/.well-known/ainm/server must be
                                // reachable before peers know each other's fingerprints).
                                // The relay handler enforces FAUT-02 at the application layer.
                                let (_, server_conn) = tls_stream.get_ref();
                                let peer_certs = server_conn.peer_certificates();

                                if let Some(certs) = peer_certs {
                                    if let Some(first_cert) = certs.first() {
                                        let peer_fingerprint =
                                            tls::compute_cert_fingerprint(first_cert.as_ref());

                                        match identity::verify_tofu(
                                            &persistence_conn,
                                            &cache,
                                            &peer_fingerprint,
                                        ) {
                                            Ok(server_id) => {
                                                tracing::info!(
                                                    "[federation] FAUT-02+FAUT-04 OK: {} (fingerprint {}) from {}",
                                                    server_id, peer_fingerprint, addr
                                                );
                                            }
                                            Err(reason) => {
                                                // Allow connection for identity/discovery endpoints
                                                // FAUT-02 enforcement happens in relay_handler
                                                tracing::warn!(
                                                    "[federation] FAUT-02 unverified peer: {} from {} (allowing for identity bootstrap)",
                                                    reason, addr
                                                );
                                            }
                                        }
                                    }
                                }
                                // --- End FAUT-02 + FAUT-04 ---

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

    // 12b. Start background cleanup task (D-12: hourly dedup + queue retention cleanup)
    let cleanup_persistence = persistence.clone();
    let _cleanup_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            // Clean dedup cache: remove entries older than 10 minutes (generous beyond 5-min window)
            match cleanup_persistence.cleanup_expired_dedup(chrono::Duration::minutes(10)) {
                Ok(n) if n > 0 => tracing::info!("[cleanup] Removed {} expired dedup entries", n),
                Err(e) => tracing::error!("[cleanup] Dedup cleanup failed: {}", e),
                _ => {}
            }
            // Clean offline queue: remove entries older than 7 days (D-12: 7-day retention)
            match cleanup_persistence.cleanup_expired_queue(chrono::Duration::days(7)) {
                Ok(n) if n > 0 => tracing::info!("[cleanup] Removed {} expired queue entries", n),
                Err(e) => tracing::error!("[cleanup] Queue cleanup failed: {}", e),
                _ => {}
            }
        }
    });

    // 13. Print federation banner
    println!();
    println!("========================================");
    println!("  AINM Federation v0.1.0");
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
                            match method {
                                "ainm.message.deliver" => {
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

                                        // Parse message text for scenario command keywords (D-11)
                                        let response_lower = text.trim().to_lowercase();
                                        let origin =
                                            federation::parse_origin_domain(thread_id).unwrap_or("");

                                        if response_lower.contains("stream") || response_lower.contains("hello") {
                                            // Streaming response via relay (Pitfall 2: emit events directly, no WS needed)
                                            let stream_events = build_stream_events(
                                                &local_identity,
                                                thread_id,
                                                &format!(
                                                    "Hello from {}! I am streaming this response token by token across the federation boundary. Each delta arrives as a separate relay payload, proving real-time streaming works across servers.",
                                                    local_identity.address
                                                ),
                                            );

                                            for event in stream_events {
                                                if origin != server_id {
                                                    relay_sender.enqueue(origin, event.clone());
                                                } else {
                                                    let deliver = serde_json::json!({
                                                        "jsonrpc": "2.0",
                                                        "method": event.get("method").and_then(|m| m.as_str()).unwrap_or("ainm.stream.delta"),
                                                        "params": event.get("params").cloned().unwrap_or_default(),
                                                    });
                                                    let _ = local_delivery_tx.send(serde_json::to_string(&deliver).unwrap());
                                                }
                                                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                                            }
                                            tracing::info!("[agent] Streamed response for thread {}", thread_id);

                                        } else if response_lower.contains("weather") || response_lower.contains("tool") {
                                            // Tool call events via relay
                                            let tool_events = build_tool_call_events(
                                                &local_identity,
                                                thread_id,
                                            );

                                            for (event, delay_ms) in tool_events {
                                                if origin != server_id {
                                                    relay_sender.enqueue(origin, event.clone());
                                                } else {
                                                    let deliver = serde_json::json!({
                                                        "jsonrpc": "2.0",
                                                        "method": "ainm.message.deliver",
                                                        "params": { "message": event, "verified": true }
                                                    });
                                                    let _ = local_delivery_tx.send(serde_json::to_string(&deliver).unwrap());
                                                }
                                                if delay_ms > 0 {
                                                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                                                }
                                            }
                                            tracing::info!("[agent] Tool call events sent for thread {}", thread_id);

                                        } else if response_lower.contains("memory") || response_lower.contains("context") {
                                            // Memory store + context frame push
                                            let response_text_msg = format!(
                                                "Memory context active for this thread. I have {} context frames available. Storing an example fact now...",
                                                1 // simplified for this scenario
                                            );

                                            let mut store_envelope = serde_json::json!({
                                                "id": protocol::new_message_id(),
                                                "from": local_identity.address,
                                                "thread_id": thread_id,
                                                "timestamp": protocol::now_rfc3339(),
                                                "type": "message",
                                                "content": [{
                                                    "type": "text",
                                                    "version": "1.0",
                                                    "source": "agent",
                                                    "text": response_text_msg,
                                                    "fallback": response_text_msg
                                                }]
                                            });
                                            if let Err(e) = signing::sign_envelope(&local_identity.key_pair, &mut store_envelope) {
                                                tracing::error!("[agent] Signing error: {}", e);
                                                continue;
                                            }

                                            if origin != server_id {
                                                relay_sender.enqueue(origin, store_envelope.clone());
                                            } else {
                                                let deliver = serde_json::json!({
                                                    "jsonrpc": "2.0",
                                                    "method": "ainm.message.deliver",
                                                    "params": { "message": store_envelope, "verified": true }
                                                });
                                                let _ = local_delivery_tx.send(serde_json::to_string(&deliver).unwrap());
                                            }
                                            tracing::info!("[agent] Memory response sent for thread {}", thread_id);

                                        } else {
                                            // Default: echo response (existing behavior)
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

                                            if origin != server_id {
                                                relay_sender.enqueue(origin, response_envelope.clone());
                                                tracing::info!(
                                                    "[agent] Response relayed to origin: {}",
                                                    origin
                                                );
                                            } else {
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
                                "ainm.context.frame" => {
                                    // D-07/D-08: Receive and log context frames pushed by origin server
                                    if let Some(params) = parsed.get("params") {
                                        let thread_id = params
                                            .get("thread_id")
                                            .and_then(|t| t.as_str())
                                            .unwrap_or("");
                                        let frames = params
                                            .get("context_frames")
                                            .and_then(|f| f.as_array());
                                        if let Some(frames) = frames {
                                            tracing::info!(
                                                "[agent] Received {} context frames for thread {}",
                                                frames.len(),
                                                thread_id
                                            );
                                            for frame in frames {
                                                let key = frame
                                                    .get("key")
                                                    .and_then(|k| k.as_str())
                                                    .unwrap_or("unknown");
                                                tracing::info!(
                                                    "[agent] Context frame: key={}",
                                                    key
                                                );
                                            }
                                        }
                                    }
                                }
                                _ => {
                                    // Ignore other notifications
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
        tls_fingerprints: body
            .get("tls_fingerprints")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
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

async fn run_reference_scenario() -> Result<()> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .init();

    // 1. Generate Ed25519 key pairs for human and agent
    let human_identity = identity::generate_keypair("alice@ainm.local")?;
    let agent_identity = identity::generate_keypair("helper@ainm.local")?;

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

    // 3. Build Agent Card for helper@ainm.local
    let agent_card = serde_json::json!({
        "name": "AINM Helper",
        "description": "A reference agent showcasing the AI-Native Messaging Protocol",
        "url": "https://ainm.local",
        "version": "0.1.0",
        "protocolVersion": "0.1.0",
        "capabilities": {
            "streaming": true,
            "pushNotifications": false
        },
        "skills": [
            {
                "id": "showcase",
                "name": "Protocol Showcase",
                "description": "Demonstrates streaming, forms, buttons, approvals, and tool calls"
            }
        ],
        "ainm": {
            "protocolVersion": "0.1.0",
            "address": "helper@ainm.local",
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
    println!("  AINM Protocol v0.1.0");
    println!("========================================");
    println!();
    println!("  Server:  ws://{}/ws", server_addr);
    println!("  Human:   {}", human_identity.address);
    println!("  Agent:   {}", agent_identity.address);
    println!();
    println!("  Commands:");
    println!("    hello    - Streaming text response");
    println!("    form     - FormBlock example");
    println!("    approve  - ApprovalBlock example");
    println!("    tool     - Tool visibility example");
    println!("    button   - ButtonBlock example");
    println!("    /card    - View Agent Card");
    println!("    /memory  - Memory store, query, context frame example");
    println!("    /bridge  - MCP and A2A bridge compatibility example");
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

/// Build streaming events for relay (Pitfall 2 solution: no WebSocket needed).
/// Returns Vec of JSON values representing JSON-RPC notifications.
fn build_stream_events(
    identity: &identity::Identity,
    thread_id: &str,
    text: &str,
) -> Vec<serde_json::Value> {
    let stream_id = protocol::new_message_id();
    let message_id = protocol::new_message_id();
    let mut events = Vec::new();

    // StreamStart
    let start = protocol::new_notification(
        "ainm.stream.start",
        serde_json::json!({
            "stream_id": stream_id,
            "message_id": message_id,
            "from": identity.address,
            "thread_id": thread_id,
            "timestamp": protocol::now_rfc3339(),
            "blocks": [{ "type": "text", "version": "1.0", "source": "agent" }]
        }),
    );
    events.push(serde_json::to_value(&start).unwrap());

    // Split text into word chunks and create deltas
    let words: Vec<&str> = text.split_inclusive(' ').collect();
    for chunk in words.chunks(3) {
        let delta_text: String = chunk.iter().copied().collect();
        let delta = protocol::new_notification(
            "ainm.stream.delta",
            serde_json::json!({
                "stream_id": stream_id,
                "block_index": 0,
                "delta": delta_text
            }),
        );
        events.push(serde_json::to_value(&delta).unwrap());
    }

    // StreamEnd with collapsed signed message
    let mut collapsed = serde_json::json!({
        "id": message_id,
        "from": identity.address,
        "thread_id": thread_id,
        "timestamp": protocol::now_rfc3339(),
        "type": "message",
        "content": [{
            "type": "text",
            "version": "1.0",
            "source": "agent",
            "text": text,
            "fallback": text
        }]
    });
    let _ = signing::sign_envelope(&identity.key_pair, &mut collapsed);

    let end = protocol::new_notification(
        "ainm.stream.end",
        serde_json::json!({
            "stream_id": stream_id,
            "collapsed_message": collapsed
        }),
    );
    events.push(serde_json::to_value(&end).unwrap());

    events
}

/// Build tool call events for relay.
/// Returns Vec of (event_json, delay_ms_before_next).
fn build_tool_call_events(
    identity: &identity::Identity,
    thread_id: &str,
) -> Vec<(serde_json::Value, u64)> {
    let call_id = format!("tc-{}", protocol::new_message_id());
    let mut events = Vec::new();

    // ToolCallStarted
    let mut started = serde_json::json!({
        "id": protocol::new_message_id(),
        "from": identity.address,
        "thread_id": thread_id,
        "timestamp": protocol::now_rfc3339(),
        "type": "tool.call.started",
        "content": [{ "type": "text", "version": "1.0", "source": "agent", "text": "Fetching weather data...", "fallback": "Fetching weather data..." }],
        "tool": { "call_id": call_id, "name": "weather_lookup", "description": "Fetching current weather data", "input": { "city": "San Francisco" } }
    });
    let _ = signing::sign_envelope(&identity.key_pair, &mut started);
    events.push((started, 500));

    // ToolCallProgress
    let mut progress = serde_json::json!({
        "id": protocol::new_message_id(),
        "from": identity.address,
        "thread_id": thread_id,
        "timestamp": protocol::now_rfc3339(),
        "type": "tool.call.progress",
        "content": [{ "type": "text", "version": "1.0", "source": "agent", "text": "Querying weather API...", "fallback": "Querying weather API..." }],
        "tool": { "call_id": call_id, "progress_text": "Querying weather API...", "percent_complete": 50 }
    });
    let _ = signing::sign_envelope(&identity.key_pair, &mut progress);
    events.push((progress, 500));

    // ToolCallCompleted
    let mut completed = serde_json::json!({
        "id": protocol::new_message_id(),
        "from": identity.address,
        "thread_id": thread_id,
        "timestamp": protocol::now_rfc3339(),
        "type": "tool.call.completed",
        "content": [{ "type": "text", "version": "1.0", "source": "agent", "text": "Weather lookup complete.", "fallback": "Weather lookup complete." }],
        "tool": { "call_id": call_id, "status": "success", "result_summary": "San Francisco: 18C, partly cloudy", "duration_ms": 1023 }
    });
    let _ = signing::sign_envelope(&identity.key_pair, &mut completed);
    events.push((completed, 0));

    // Follow-up message with results
    let mut result_msg = serde_json::json!({
        "id": protocol::new_message_id(),
        "from": identity.address,
        "thread_id": thread_id,
        "timestamp": protocol::now_rfc3339(),
        "type": "message",
        "content": [{ "type": "text", "version": "1.0", "source": "agent", "text": "The weather in San Francisco is 18C and partly cloudy. (Tool call events relayed across federation.)", "fallback": "San Francisco: 18C, partly cloudy." }]
    });
    let _ = signing::sign_envelope(&identity.key_pair, &mut result_msg);
    events.push((result_msg, 0));

    events
}
