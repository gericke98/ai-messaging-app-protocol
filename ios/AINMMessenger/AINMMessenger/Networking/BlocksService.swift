import Foundation

/// Bearer-authed REST client for the server's UGC safety endpoints
/// (App Store guideline 1.2): block users and report content.
///
/// Endpoints (all require a session token from ``SessionAuthService``):
/// - `PUT    /accounts/blocks/{address}` — block (idempotent, 204)
/// - `DELETE /accounts/blocks/{address}` — unblock (idempotent, 204)
/// - `GET    /accounts/blocks`           — list of blocked addresses
/// - `POST   /accounts/reports`          — file a content report (201)
///
/// Mirrors the ``AccountService`` pattern: actor-isolated, snake_case DTOs,
/// `baseURL` is the HTTP origin (caller maps ws→http and strips `/ws`).
actor BlocksService {
    private let session = URLSession.shared
    private let baseURL: URL
    private let token: String

    init(baseURL: URL, token: String) {
        self.baseURL = baseURL
        self.token = token
    }

    enum BlocksError: Error, LocalizedError {
        case http(Int, String)
        case decoding

        var errorDescription: String? {
            switch self {
            case .http(401, _): return "Your session expired. Reconnect and try again."
            case .http(let c, let m): return "Server error \(c): \(m)"
            case .decoding: return "Could not read the server response."
            }
        }
    }

    // MARK: - DTOs (snake_case JSON keys match the server's ReportBody)

    struct ReportReq: Encodable {
        let subject: String
        let thread_id: String?
        let message_id: String?
        let reason: String
    }

    /// Normalize a report reason per the server rule (trimmed, 1–2000 chars).
    /// Returns the trimmed reason, or nil when it would be rejected — the UI
    /// disables Submit on nil instead of round-tripping a 400.
    static func validReason(_ raw: String) -> String? {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed.count <= 2000 else { return nil }
        return trimmed
    }

    // MARK: - Request construction (pure; unit-tested)

    /// Build a bearer-authed request whose URL appends each path segment.
    /// AINM addresses are passed as a single segment; `@` is a legal path
    /// character so the address survives readably (axum decodes `{address}`).
    static func makeRequest(
        baseURL: URL,
        path: [String],
        method: String,
        token: String
    ) throws -> URLRequest {
        var url = baseURL
        for segment in path {
            url.appendPathComponent(segment)
        }
        var req = URLRequest(url: url)
        req.httpMethod = method
        req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        return req
    }

    // MARK: - Transport

    private func send(_ req: URLRequest) async throws -> Data {
        let (data, resp) = try await session.data(for: req)
        guard let http = resp as? HTTPURLResponse else { throw BlocksError.decoding }
        guard (200..<300).contains(http.statusCode) else {
            throw BlocksError.http(http.statusCode, String(data: data, encoding: .utf8) ?? "")
        }
        return data
    }

    // MARK: - Endpoints

    /// `PUT /accounts/blocks/{address}` — block a user (idempotent).
    func block(_ address: String) async throws {
        let req = try Self.makeRequest(
            baseURL: baseURL, path: ["accounts", "blocks", address], method: "PUT", token: token)
        _ = try await send(req)
    }

    /// `DELETE /accounts/blocks/{address}` — unblock a user (idempotent).
    func unblock(_ address: String) async throws {
        let req = try Self.makeRequest(
            baseURL: baseURL, path: ["accounts", "blocks", address], method: "DELETE", token: token)
        _ = try await send(req)
    }

    /// `GET /accounts/blocks` — the caller's blocked addresses.
    func listBlocks() async throws -> [String] {
        let req = try Self.makeRequest(
            baseURL: baseURL, path: ["accounts", "blocks"], method: "GET", token: token)
        let data = try await send(req)
        guard let list = try? JSONDecoder().decode([String].self, from: data) else {
            throw BlocksError.decoding
        }
        return list
    }

    /// `POST /accounts/reports` — report a user/message (reason 1–2000 chars).
    func report(
        subject: String,
        threadId: String?,
        messageId: String?,
        reason: String
    ) async throws {
        var req = try Self.makeRequest(
            baseURL: baseURL, path: ["accounts", "reports"], method: "POST", token: token)
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try JSONEncoder().encode(
            ReportReq(subject: subject, thread_id: threadId, message_id: messageId, reason: reason))
        _ = try await send(req)
    }
}
