import LocalAuthentication

/// Local biometric unlock gate. NOT a key substitute — purely a device-local convenience gate.
struct BiometricManager {
    enum BioError: Error, LocalizedError {
        case unavailable
        case failed
        var errorDescription: String? {
            switch self {
            case .unavailable: return "Biometrics are unavailable on this device."
            case .failed: return "Authentication failed."
            }
        }
    }

    /// Whether biometric auth can be evaluated on this device right now.
    static func isAvailable() -> Bool {
        var error: NSError?
        return LAContext().canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error)
    }

    /// Prompt Face ID / Touch ID. Returns normally on success; throws BioError on failure/unavailability.
    static func authenticate(reason: String = "Unlock AINM") async throws {
        let context = LAContext()
        var error: NSError?
        guard context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error) else {
            throw BioError.unavailable
        }
        let success = try await context.evaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, localizedReason: reason)
        if !success { throw BioError.failed }
    }
}
