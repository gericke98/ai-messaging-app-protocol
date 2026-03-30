import Foundation

/// RFC 8785 JSON Canonicalization Scheme (JCS) implementation.
///
/// Produces deterministic JSON output by:
/// 1. Sorting object keys by UTF-16 code unit comparison
/// 2. Removing all insignificant whitespace
/// 3. Serializing numbers per ECMAScript Number.toString()
/// 4. Escaping strings per RFC 8785 Section 3.2.2.2
///
/// Used for cryptographic signing -- the canonical form is the signing input.
enum JCSCanonicalizer {

    enum JCSError: Error {
        case invalidJSON
        case unsupportedType
    }

    /// Canonicalize a JSON Data blob.
    static func canonicalize(_ jsonData: Data) throws -> String {
        let value = try JSONSerialization.jsonObject(with: jsonData, options: [.fragmentsAllowed])
        return try canonicalize(value)
    }

    /// Canonicalize an already-parsed JSON value (from JSONSerialization).
    static func canonicalize(_ value: Any) throws -> String {
        var output = ""
        try serializeValue(value, to: &output)
        return output
    }

    // MARK: - Private Implementation

    private static func serializeValue(_ value: Any, to output: inout String) throws {
        switch value {
        case is NSNull:
            output += "null"

        case let number as NSNumber:
            // Distinguish Bool from Number using CFBoolean type check
            if CFGetTypeID(number) == CFBooleanGetTypeID() {
                output += number.boolValue ? "true" : "false"
            } else {
                output += serializeNumber(number)
            }

        case let string as String:
            serializeString(string, to: &output)

        case let array as [Any]:
            output += "["
            for (index, element) in array.enumerated() {
                if index > 0 { output += "," }
                try serializeValue(element, to: &output)
            }
            output += "]"

        case let dict as [String: Any]:
            output += "{"
            // Sort keys by UTF-16 code units (RFC 8785 Section 3.2.3)
            let sortedKeys = dict.keys.sorted { a, b in
                a.utf16.lexicographicallyPrecedes(b.utf16)
            }
            for (index, key) in sortedKeys.enumerated() {
                if index > 0 { output += "," }
                serializeString(key, to: &output)
                output += ":"
                try serializeValue(dict[key]!, to: &output)
            }
            output += "}"

        default:
            throw JCSError.unsupportedType
        }
    }

    /// Serialize a number per ECMAScript Number.toString() rules.
    /// - Integer values: no decimal point (1, not 1.0)
    /// - Fractional values: minimal representation (1.5, not 1.50)
    /// - Negative zero: "0" (not "-0")
    private static func serializeNumber(_ number: NSNumber) -> String {
        let doubleValue = number.doubleValue

        // Handle negative zero -> "0"
        if doubleValue.isZero && doubleValue.sign == .minus {
            return "0"
        }

        // Check if the value is an integer (no fractional part)
        // Use the objCType to check if it was originally an integer type
        let objCType = String(cString: number.objCType)
        let integerTypes: Set<String> = ["c", "C", "s", "S", "i", "I", "l", "L", "q", "Q"]

        if integerTypes.contains(objCType) {
            // Integer type -- serialize without decimal
            return "\(number.int64Value)"
        }

        // For floating point: check if it's a whole number
        if doubleValue == doubleValue.rounded(.towardZero) && !doubleValue.isInfinite {
            let intValue = Int64(doubleValue)
            if Double(intValue) == doubleValue {
                return "\(intValue)"
            }
        }

        // Fractional number -- use minimal representation
        // ECMAScript spec: shortest representation that round-trips
        return formatDouble(doubleValue)
    }

    /// Format a double to its shortest round-trippable decimal representation.
    private static func formatDouble(_ value: Double) -> String {
        // Use Swift's default Double formatting which matches ECMAScript for most cases
        let str = "\(value)"

        // Remove trailing zeros after decimal point, but keep at least one digit
        if str.contains(".") {
            var result = str
            while result.hasSuffix("0") {
                result.removeLast()
            }
            if result.hasSuffix(".") {
                result.removeLast()
            }
            return result
        }
        return str
    }

    /// Serialize a string with RFC 8785 Section 3.2.2.2 escaping.
    /// Escape: \b (0x08), \t (0x09), \n (0x0A), \f (0x0C), \r (0x0D),
    ///         \" (0x22), \\ (0x5C), and all control chars 0x00-0x1F as \uXXXX.
    private static func serializeString(_ string: String, to output: inout String) {
        output += "\""
        for scalar in string.unicodeScalars {
            let value = scalar.value
            switch value {
            case 0x08:
                output += "\\b"
            case 0x09:
                output += "\\t"
            case 0x0A:
                output += "\\n"
            case 0x0C:
                output += "\\f"
            case 0x0D:
                output += "\\r"
            case 0x22:
                output += "\\\""
            case 0x5C:
                output += "\\\\"
            case 0x00...0x1F:
                // Other control characters as \uXXXX
                output += String(format: "\\u%04x", value)
            default:
                // All other characters pass through unescaped (including Unicode)
                output += String(scalar)
            }
        }
        output += "\""
    }
}
