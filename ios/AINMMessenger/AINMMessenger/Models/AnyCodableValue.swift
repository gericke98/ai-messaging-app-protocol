import Foundation

/// Type-erased JSON value for dynamic JSON-RPC params and results.
/// Supports all JSON value types: null, bool, int, double, string, array, object.
enum AnyCodableValue: Codable, Sendable, Equatable {
    case null
    case bool(Bool)
    case int(Int)
    case double(Double)
    case string(String)
    case array([AnyCodableValue])
    case object([String: AnyCodableValue])

    // MARK: - Codable

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()

        if container.decodeNil() {
            self = .null
            return
        }

        // Try bool before int since Bool conforms to Int in some contexts
        if let boolValue = try? container.decode(Bool.self) {
            self = .bool(boolValue)
            return
        }

        if let intValue = try? container.decode(Int.self) {
            self = .int(intValue)
            return
        }

        if let doubleValue = try? container.decode(Double.self) {
            self = .double(doubleValue)
            return
        }

        if let stringValue = try? container.decode(String.self) {
            self = .string(stringValue)
            return
        }

        if let arrayValue = try? container.decode([AnyCodableValue].self) {
            self = .array(arrayValue)
            return
        }

        if let objectValue = try? container.decode([String: AnyCodableValue].self) {
            self = .object(objectValue)
            return
        }

        throw DecodingError.dataCorruptedError(
            in: container,
            debugDescription: "Unable to decode AnyCodableValue"
        )
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .null:
            try container.encodeNil()
        case .bool(let value):
            try container.encode(value)
        case .int(let value):
            try container.encode(value)
        case .double(let value):
            try container.encode(value)
        case .string(let value):
            try container.encode(value)
        case .array(let value):
            try container.encode(value)
        case .object(let value):
            try container.encode(value)
        }
    }

    // MARK: - Convenience Initializers

    /// Create from any Encodable value by round-tripping through JSON.
    init(_ encodable: some Encodable) throws {
        let data = try JSONEncoder().encode(encodable)
        self = try JSONDecoder().decode(AnyCodableValue.self, from: data)
    }

    /// Convert an untyped `Any` value (e.g. from `[String: Any]`) to `AnyCodableValue`.
    /// Used by the ActionBlock flow where onAction closures pass `[String: Any]` data.
    static func fromAny(_ value: Any) -> AnyCodableValue {
        if value is NSNull {
            return .null
        }
        if let bool = value as? Bool {
            return .bool(bool)
        }
        if let int = value as? Int {
            return .int(int)
        }
        if let double = value as? Double {
            return .double(double)
        }
        if let string = value as? String {
            return .string(string)
        }
        if let array = value as? [Any] {
            return .array(array.map { fromAny($0) })
        }
        if let dict = value as? [String: Any] {
            return .object(dict.mapValues { fromAny($0) })
        }
        // Fallback: attempt string conversion
        return .string(String(describing: value))
    }
}

// MARK: - ExpressibleBy Literals

extension AnyCodableValue: ExpressibleByNilLiteral {
    init(nilLiteral: ()) {
        self = .null
    }
}

extension AnyCodableValue: ExpressibleByBooleanLiteral {
    init(booleanLiteral value: Bool) {
        self = .bool(value)
    }
}

extension AnyCodableValue: ExpressibleByIntegerLiteral {
    init(integerLiteral value: Int) {
        self = .int(value)
    }
}

extension AnyCodableValue: ExpressibleByFloatLiteral {
    init(floatLiteral value: Double) {
        self = .double(value)
    }
}

extension AnyCodableValue: ExpressibleByStringLiteral {
    init(stringLiteral value: String) {
        self = .string(value)
    }
}

extension AnyCodableValue: ExpressibleByArrayLiteral {
    init(arrayLiteral elements: AnyCodableValue...) {
        self = .array(elements)
    }
}

extension AnyCodableValue: ExpressibleByDictionaryLiteral {
    init(dictionaryLiteral elements: (String, AnyCodableValue)...) {
        self = .object(Dictionary(uniqueKeysWithValues: elements))
    }
}
