import Foundation

// MARK: - JSON-RPC 2.0 Types
// Matches demo/src/protocol.rs JSON-RPC types

/// JSON-RPC 2.0 request (client -> server).
struct JsonRpcRequest: Codable, Sendable {
    let jsonrpc: String // Always "2.0"
    let id: AnyCodableValue
    let method: String
    let params: AnyCodableValue?

    private enum CodingKeys: String, CodingKey {
        case jsonrpc, id, method, params
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(jsonrpc, forKey: .jsonrpc)
        try container.encode(id, forKey: .id)
        try container.encode(method, forKey: .method)
        // Skip nil params to match Rust's skip_serializing_if = "Option::is_none"
        if let params {
            try container.encode(params, forKey: .params)
        }
    }
}

/// JSON-RPC 2.0 response (server -> client).
struct JsonRpcResponse: Codable, Sendable {
    let jsonrpc: String // Always "2.0"
    let id: AnyCodableValue
    let result: AnyCodableValue?
    let error: JsonRpcError?

    private enum CodingKeys: String, CodingKey {
        case jsonrpc, id, result, error
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(jsonrpc, forKey: .jsonrpc)
        try container.encode(id, forKey: .id)
        if let result {
            try container.encode(result, forKey: .result)
        }
        if let error {
            try container.encode(error, forKey: .error)
        }
    }
}

/// JSON-RPC 2.0 notification (no id, no response expected).
struct JsonRpcNotification: Codable, Sendable {
    let jsonrpc: String // Always "2.0"
    let method: String
    let params: AnyCodableValue?

    private enum CodingKeys: String, CodingKey {
        case jsonrpc, method, params
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(jsonrpc, forKey: .jsonrpc)
        try container.encode(method, forKey: .method)
        if let params {
            try container.encode(params, forKey: .params)
        }
    }
}

/// JSON-RPC 2.0 error object.
struct JsonRpcError: Codable, Sendable {
    let code: Int
    let message: String
    let data: AnyCodableValue?

    private enum CodingKeys: String, CodingKey {
        case code, message, data
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(code, forKey: .code)
        try container.encode(message, forKey: .message)
        if let data {
            try container.encode(data, forKey: .data)
        }
    }
}

// MARK: - Factory Methods

extension JsonRpcRequest {
    /// Create an ainm.initialize request.
    static func initialize(id: AnyCodableValue, params: InitializeParams) throws -> JsonRpcRequest {
        let paramsValue = try AnyCodableValue(params)
        return JsonRpcRequest(
            jsonrpc: "2.0",
            id: id,
            method: "ainm.initialize",
            params: paramsValue
        )
    }
}

extension JsonRpcNotification {
    /// Create an ainm.initialized notification (client -> server after receiving initialize result).
    static func initialized() -> JsonRpcNotification {
        JsonRpcNotification(
            jsonrpc: "2.0",
            method: "ainm.initialized",
            params: nil
        )
    }
}

// MARK: - AINM Error Codes

extension JsonRpcError {
    // Security errors (-32010 to -32013)
    static let invalidSignature = -32010
    static let unknownSender = -32011
    static let revokedKey = -32012
    static let malformedMessage = -32013

    // Transport errors (-32020 to -32031)
    static let unsupportedProtocolVersion = -32020
    static let notInitialized = -32021
    static let rateLimited = -32022
    static let serverUnavailable = -32023
    static let threadNotFound = -32024
    static let notAParticipant = -32025
    static let replayDetected = -32026
    static let threadCreationFailed = -32030
    static let alreadyAParticipant = -32031
}
