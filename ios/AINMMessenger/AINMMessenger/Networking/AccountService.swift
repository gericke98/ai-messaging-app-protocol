import Foundation

/// URLSession-based REST client for the AINM server's account endpoints.
///
/// Mirrors the ``AgentCardService`` pattern: actor-isolated, async/await,
/// typed Codable DTOs, and a `LocalizedError` enum.
///
/// All DTO JSON keys are snake_case to match the server exactly. The default
/// `JSONEncoder`/`JSONDecoder` (no `keyEncodingStrategy`) is used, so the Swift
/// property names are emitted verbatim as JSON keys.
///
/// `baseURL` is an HTTP base (e.g. `http://localhost:8080`). The caller is
/// responsible for converting the stored WebSocket `serverURL` (ws→http) and
/// dropping the `/ws` path before constructing this service. AccountService
/// only appends endpoint paths to `baseURL`.
actor AccountService {
    private let session = URLSession.shared
    private let baseURL: URL

    init(baseURL: URL) {
        self.baseURL = baseURL
    }

    enum AccountError: Error, LocalizedError {
        case http(Int, String)
        case decoding
        case escrowDestroyed

        var errorDescription: String? {
            switch self {
            case .http(let c, let m): return "Server error \(c): \(m)"
            case .decoding: return "Could not read the server response."
            case .escrowDestroyed: return "Recovery is no longer possible (too many attempts)."
            }
        }
    }

    // MARK: - DTOs (snake_case JSON keys match the server exactly)

    struct SignupReq: Encodable { let email: String; let username: String }
    struct SignupResp: Decodable { let address: String }

    struct VerifyReq: Encodable { let email: String; let code: String }
    struct VerifyResp: Decodable { let enrollment_token: String }

    struct BindKeyReq: Encodable {
        let enrollment_token: String
        let public_key: String
        let device_label: String?
    }

    struct KdfParams: Codable {
        let salt: String
        let m_cost: UInt32
        let t_cost: UInt32
        let p_cost: UInt32
    }

    struct StoreEscrowReq: Encodable {
        let enrollment_token: String
        let ciphertext_b64: String
        let kdf_params: KdfParams
        let oprf_key_b64: String
    }

    struct RecoverStartReq: Encodable { let email: String }

    struct RecoverVerifyReq: Encodable { let email: String; let code: String }
    struct RecoverVerifyResp: Decodable { let recovery_token: String }

    struct EscrowFetchReq: Encodable {
        let recovery_token: String
        let blinded_point: String
    }

    struct EscrowFetchResp: Decodable {
        let evaluated_point: String
        let ciphertext_b64: String
        let kdf_params: KdfParams
    }

    private struct EmptyResp: Decodable {}

    // MARK: - Transport

    /// POST `body` as JSON to `path` (relative to `baseURL`) and decode the response.
    ///
    /// - 410 Gone maps to ``AccountError/escrowDestroyed``.
    /// - Non-2xx maps to ``AccountError/http(_:_:)`` with the response body.
    /// - For ``EmptyResp`` (204 / empty-body endpoints) no decoding is attempted.
    private func post<Req: Encodable, Resp: Decodable>(_ path: String, _ body: Req) async throws -> Resp {
        var req = URLRequest(url: baseURL.appendingPathComponent(path))
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try JSONEncoder().encode(body)

        let (data, resp) = try await session.data(for: req)
        guard let http = resp as? HTTPURLResponse else { throw AccountError.decoding }

        if http.statusCode == 410 { throw AccountError.escrowDestroyed }
        guard (200..<300).contains(http.statusCode) else {
            throw AccountError.http(http.statusCode, String(data: data, encoding: .utf8) ?? "")
        }

        if Resp.self == EmptyResp.self { return EmptyResp() as! Resp }
        do {
            return try JSONDecoder().decode(Resp.self, from: data)
        } catch {
            throw AccountError.decoding
        }
    }

    // MARK: - Endpoints

    /// `POST /accounts/signup` → returns the assigned AINM address.
    func signup(email: String, username: String) async throws -> String {
        let r: SignupResp = try await post("accounts/signup", SignupReq(email: email, username: username))
        return r.address
    }

    /// `POST /accounts/verify` → returns the enrollment token.
    func verify(email: String, code: String) async throws -> String {
        let r: VerifyResp = try await post("accounts/verify", VerifyReq(email: email, code: code))
        return r.enrollment_token
    }

    /// `POST /accounts/keys` → binds a device public key (204, no body).
    func bindKey(enrollmentToken: String, publicKeyB64: String, deviceLabel: String?) async throws {
        let _: EmptyResp = try await post(
            "accounts/keys",
            BindKeyReq(enrollment_token: enrollmentToken, public_key: publicKeyB64, device_label: deviceLabel)
        )
    }

    /// `POST /accounts/escrow` → stores the OPRF-protected key escrow (204, no body).
    func storeEscrow(enrollmentToken: String, ciphertextB64: String, kdf: KdfParams, oprfKeyB64: String) async throws {
        let _: EmptyResp = try await post(
            "accounts/escrow",
            StoreEscrowReq(enrollment_token: enrollmentToken, ciphertext_b64: ciphertextB64, kdf_params: kdf, oprf_key_b64: oprfKeyB64)
        )
    }

    /// `POST /accounts/recover/start` → begins recovery; always 204.
    func recoverStart(email: String) async throws {
        let _: EmptyResp = try await post("accounts/recover/start", RecoverStartReq(email: email))
    }

    /// `POST /accounts/recover/verify` → returns the recovery token.
    func recoverVerify(email: String, code: String) async throws -> String {
        let r: RecoverVerifyResp = try await post("accounts/recover/verify", RecoverVerifyReq(email: email, code: code))
        return r.recovery_token
    }

    /// `POST /accounts/escrow/fetch` → returns the OPRF-evaluated escrow material.
    /// Throws ``AccountError/escrowDestroyed`` on 410 Gone.
    func escrowFetch(recoveryToken: String, blindedPoint: String) async throws -> EscrowFetchResp {
        try await post("accounts/escrow/fetch", EscrowFetchReq(recovery_token: recoveryToken, blinded_point: blindedPoint))
    }
}
