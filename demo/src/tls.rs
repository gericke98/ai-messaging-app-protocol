//! TLS configuration for AINM federation mutual TLS (mTLS).
//!
//! Provides builders for rustls ServerConfig (inbound federation) and
//! reqwest Client (outbound federation), plus certificate fingerprint
//! computation per FAUT-02.

use rustls::server::WebPkiClientVerifier;
use rustls::ServerConfig;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use std::sync::Arc;

/// Build a rustls `ServerConfig` that requires mutual TLS.
///
/// The CA certificate is used to verify connecting clients (other AINM servers).
/// The server certificate and key identify this server to connecting clients.
pub fn build_mtls_server_config(
    ca_cert_path: &str,
    server_cert_path: &str,
    server_key_path: &str,
) -> Arc<ServerConfig> {
    // Load CA certificates for client verification
    let ca_certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(ca_cert_path)
        .expect("failed to open CA cert file")
        .collect::<Result<Vec<_>, _>>()
        .expect("failed to parse CA certs");

    let mut root_store = rustls::RootCertStore::empty();
    for cert in ca_certs {
        root_store.add(cert).expect("failed to add CA cert to root store");
    }

    // Build client verifier requiring valid client certificate
    let client_verifier = WebPkiClientVerifier::builder(Arc::new(root_store))
        .build()
        .expect("failed to build client verifier");

    // Load this server's certificate chain
    let server_certs: Vec<CertificateDer<'static>> =
        CertificateDer::pem_file_iter(server_cert_path)
            .expect("failed to open server cert file")
            .collect::<Result<Vec<_>, _>>()
            .expect("failed to parse server certs");

    // Load this server's private key
    let server_key = PrivateKeyDer::from_pem_file(server_key_path)
        .expect("failed to load server private key");

    // Build the ServerConfig with mTLS
    let mut config = ServerConfig::builder()
        .with_client_cert_verifier(client_verifier)
        .with_single_cert(server_certs, server_key)
        .expect("failed to build ServerConfig with cert/key");

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Arc::new(config)
}

/// Build a reqwest `Client` configured for outbound mTLS.
///
/// The CA certificate is used to verify the remote server.
/// The client certificate and key identify this server to the remote.
///
/// Uses a manually-configured rustls ClientConfig with webpki verification
/// (not rustls-platform-verifier) so that our demo CA cert is sufficient
/// without being added to the macOS system trust store.
pub fn build_mtls_client(
    ca_cert_path: &str,
    client_cert_path: &str,
    client_key_path: &str,
) -> reqwest::Client {
    // Load CA certs for server verification (webpki root store)
    let ca_certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(ca_cert_path)
        .expect("failed to open CA cert file")
        .collect::<Result<Vec<_>, _>>()
        .expect("failed to parse CA certs");

    let mut root_store = rustls::RootCertStore::empty();
    for cert in ca_certs {
        root_store.add(cert).expect("failed to add CA cert to root store");
    }

    // Load client cert chain and key for mTLS
    let client_certs: Vec<CertificateDer<'static>> =
        CertificateDer::pem_file_iter(client_cert_path)
            .expect("failed to open client cert file")
            .collect::<Result<Vec<_>, _>>()
            .expect("failed to parse client certs");

    let client_key = PrivateKeyDer::from_pem_file(client_key_path)
        .expect("failed to load client private key");

    // Build rustls ClientConfig with webpki verifier (bypasses platform verifier)
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_client_auth_cert(client_certs, client_key)
        .expect("failed to build ClientConfig with client cert");

    reqwest::Client::builder()
        .use_preconfigured_tls(config)
        .build()
        .expect("failed to build mTLS reqwest client")
}

/// Compute a SHA-256 fingerprint of a DER-encoded certificate.
///
/// Returns a string in the format `"sha256:hexhex..."` (lowercase, no separators).
/// Used for FAUT-02 certificate fingerprint verification.
pub fn compute_cert_fingerprint(der_bytes: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, der_bytes);
    let hex: String = digest.as_ref().iter().map(|b| format!("{:02x}", b)).collect();
    format!("sha256:{}", hex)
}

/// Load the first certificate from a PEM file and return its DER bytes.
///
/// Helper for computing fingerprints from PEM files:
/// ```ignore
/// let der = load_cert_der("demo/certs/server-a.pem");
/// let fp = compute_cert_fingerprint(&der);
/// ```
pub fn load_cert_der(pem_path: &str) -> Vec<u8> {
    let cert = CertificateDer::pem_file_iter(pem_path)
        .expect("failed to open PEM file")
        .next()
        .expect("PEM file contains no certificates")
        .expect("failed to parse certificate from PEM");
    cert.as_ref().to_vec()
}
