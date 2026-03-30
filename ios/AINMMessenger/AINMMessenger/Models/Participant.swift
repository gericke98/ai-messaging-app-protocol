import Foundation

/// Participant in a thread.
/// Matches threading.schema.json participant model.
struct Participant: Codable, Sendable {
    let identity: String
    let type: ParticipantType
    let joinedAt: String
    let role: ParticipantRole

    enum ParticipantType: String, Codable, Sendable {
        case human, agent
    }

    enum ParticipantRole: String, Codable, Sendable {
        case creator, member
    }

    enum CodingKeys: String, CodingKey {
        case identity
        case type
        case joinedAt = "joined_at"
        case role
    }
}
