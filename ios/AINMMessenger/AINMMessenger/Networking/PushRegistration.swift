import Foundation
import UserNotifications
#if canImport(UIKit)
import UIKit
#endif

/// APNs registration plumbing (M4 push notifications).
///
/// Pure helpers (hex token, request shape, payload parsing) are unit-tested;
/// `PushDelegate` is the thin UIKit adaptor receiving the APNs callbacks.
/// NOTE: live registration additionally requires the Push Notifications
/// capability (`aps-environment` entitlement) under a real provisioning
/// profile — part of the Apple Developer account setup at submission time.
/// Until then `didFailToRegister` fires and is logged; nothing else breaks.
enum PushRegistration {

    /// APNs hands the device token over as raw bytes; the server stores it
    /// as lowercase hex (the conventional representation).
    static func hexToken(_ data: Data) -> String {
        data.map { String(format: "%02x", $0) }.joined()
    }

    /// Build the bearer-authed `PUT /accounts/push-token` request (pure; unit-tested).
    static func makeRegisterRequest(
        baseURL: URL, bearer: String, deviceTokenHex: String
    ) throws -> URLRequest {
        var req = URLRequest(url: baseURL.appendingPathComponent("accounts/push-token"))
        req.httpMethod = "PUT"
        req.setValue("Bearer \(bearer)", forHTTPHeaderField: "Authorization")
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try JSONSerialization.data(withJSONObject: [
            "device_token": deviceTokenHex,
            "platform": "ios",
        ])
        return req
    }

    /// The server's APNs payload carries a top-level `thread_id` for deep-linking.
    static func threadId(fromUserInfo userInfo: [AnyHashable: Any]) -> String? {
        userInfo["thread_id"] as? String
    }
}

#if canImport(UIKit)
/// UIApplicationDelegate adaptor: receives the APNs device token and
/// notification taps, forwarding both to AppState on the main actor.
final class PushDelegate: NSObject, UIApplicationDelegate, UNUserNotificationCenterDelegate {
    /// Wired by AINMMessengerApp at launch.
    weak var appState: AppState?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        UNUserNotificationCenter.current().delegate = self
        return true
    }

    func application(
        _ application: UIApplication,
        didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data
    ) {
        let hex = PushRegistration.hexToken(deviceToken)
        Task { @MainActor in
            await self.appState?.pushTokenReceived(hex)
        }
    }

    func application(
        _ application: UIApplication,
        didFailToRegisterForRemoteNotificationsWithError error: Error
    ) {
        // Expected until the aps-environment entitlement exists (Apple dev
        // account setup); harmless on simulator.
        print("[push] APNs registration failed: \(error.localizedDescription)")
    }

    /// Foreground delivery: suppress the banner — the live WebSocket already
    /// renders the message in-app. (nonisolated: the delegate protocol calls
    /// off the main actor; nothing here touches actor state.)
    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) ->
            Void
    ) {
        completionHandler([])
    }

    /// Notification tap → deep-link into the conversation. The thread id is
    /// extracted off-actor (a plain String), then applied on the main actor.
    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        let threadId = PushRegistration.threadId(
            fromUserInfo: response.notification.request.content.userInfo)
        if let threadId {
            Task { @MainActor [weak self] in
                self?.appState?.selectedThreadId = threadId
            }
        }
        completionHandler()
    }
}
#endif
