import CryptoKit
import Foundation

/// Mints bearer session tokens via `POST /accounts/auth/token` by proving possession
/// of the account's bound Ed25519 key (domain-separated challenge, see
/// ``MessageSigner/signAuthChallenge(identity:timestamp:privateKey:)``).
///
/// Mirrors the ``AccountService`` pattern: actor-isolated, snake_case DTOs encoded
/// with the default strategy, `baseURL` is the HTTP origin (caller maps ws→http).
/// The minted token authorizes every authed REST endpoint: blocks, reports,
/// session management, and account deletion.
actor SessionAuthService {
    private let session = URLSession.shared
    private let baseURL: URL

    init(baseURL: URL) {
        self.baseURL = baseURL
    }

    enum SessionAuthError: Error, LocalizedError {
        case http(Int, String)
        case decoding

        var errorDescription: String? {
            switch self {
            case .http(401, _): return "The server rejected this device's key. Try signing in again."
            case .http(let c, let m): return "Server error \(c): \(m)"
            case .decoding: return "Could not read the server response."
            }
        }
    }

    // MARK: - DTOs (snake_case JSON keys match the server's MintTokenReq/Resp)

    struct MintReq: Encodable {
        let identity: String
        let timestamp: String
        let signature: String
        let device_label: String?
    }

    struct MintResp: Decodable {
        let token: String
        let session_id: String
        let expires_at: String
    }

    /// A minted session: the bearer token plus its expiry for re-mint scheduling.
    struct SessionToken: Sendable {
        let token: String
        let expiresAt: Date

        /// Usable until 60s before expiry — the margin keeps a request from
        /// leaving the device with a token that expires mid-flight.
        func isValid(now: Date) -> Bool {
            now < expiresAt.addingTimeInterval(-60)
        }
    }

    // MARK: - Request construction (pure; unit-tested)

    /// Build the mint request body: RFC 3339 timestamp for `now` and an Ed25519
    /// signature over the server's challenge payload
    /// `"ainm-auth-token|{identity}|{timestamp}"`.
    static func makeMintRequest(
        identity: String,
        privateKey: Curve25519.Signing.PrivateKey,
        deviceLabel: String?,
        now: Date = Date()
    ) throws -> MintReq {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        let timestamp = formatter.string(from: now)
        let signature = try MessageSigner.signAuthChallenge(
            identity: identity, timestamp: timestamp, privateKey: privateKey)
        return MintReq(
            identity: identity, timestamp: timestamp,
            signature: signature, device_label: deviceLabel)
    }

    // MARK: - Endpoint

    /// `POST /accounts/auth/token` → bearer token + expiry. Uniform 401 on any
    /// auth failure (unknown account, no active key, bad signature, stale timestamp).
    func mint(
        identity: String,
        privateKey: Curve25519.Signing.PrivateKey,
        deviceLabel: String?
    ) async throws -> SessionToken {
        let body = try Self.makeMintRequest(
            identity: identity, privateKey: privateKey, deviceLabel: deviceLabel)

        var req = URLRequest(url: baseURL.appendingPathComponent("accounts/auth/token"))
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try JSONEncoder().encode(body)

        let (data, resp) = try await session.data(for: req)
        guard let http = resp as? HTTPURLResponse else { throw SessionAuthError.decoding }
        guard (200..<300).contains(http.statusCode) else {
            throw SessionAuthError.http(http.statusCode, String(data: data, encoding: .utf8) ?? "")
        }

        guard let decoded = try? JSONDecoder().decode(MintResp.self, from: data),
            let expiresAt = ISO8601DateFormatter().date(from: decoded.expires_at)
        else { throw SessionAuthError.decoding }
        return SessionToken(token: decoded.token, expiresAt: expiresAt)
    }
}
