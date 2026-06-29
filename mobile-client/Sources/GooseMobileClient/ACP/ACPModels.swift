import Foundation

public enum ACPMethod {
    public static let initialize = "initialize"
    public static let listSessions = "session/list"
    public static let loadSession = "session/load"
    public static let prompt = "session/prompt"
    public static let cancel = "session/cancel"
    public static let sessionUpdate = "session/update"
    public static let requestPermission = "session/request_permission"
    public static let gooseSessionInfo = "_goose/unstable/session/info"
}

public struct ImplementationInfo: Codable, Equatable, Sendable {
    public var name: String
    public var title: String?
    public var version: String
}

public struct InitializeResponse: Codable, Equatable, Sendable {
    public var protocolVersion: Int
    public var agentInfo: ImplementationInfo?
    public var agentCapabilities: JSONValue?
    public var authMethods: [JSONValue]?
    public var metadata: [String: JSONValue]?

    enum CodingKeys: String, CodingKey {
        case protocolVersion
        case agentInfo
        case agentCapabilities
        case authMethods
        case metadata = "_meta"
    }
}

public struct SessionInfo: Codable, Equatable, Identifiable, Sendable {
    public var sessionID: String
    public var cwd: String
    public var additionalDirectories: [String]?
    public var title: String?
    public var updatedAt: String?
    public var metadata: [String: JSONValue]?

    public var id: String { sessionID }

    enum CodingKeys: String, CodingKey {
        case sessionID = "sessionId"
        case cwd
        case additionalDirectories
        case title
        case updatedAt
        case metadata = "_meta"
    }
}

public struct ListSessionsResponse: Codable, Equatable, Sendable {
    public var sessions: [SessionInfo]
    public var nextCursor: String?
    public var metadata: [String: JSONValue]?

    enum CodingKeys: String, CodingKey {
        case sessions
        case nextCursor
        case metadata = "_meta"
    }
}

public struct LoadSessionResponse: Codable, Equatable, Sendable {
    public var configOptions: JSONValue?
    public var models: JSONValue?
    public var modes: JSONValue?
    public var metadata: [String: JSONValue]?

    enum CodingKeys: String, CodingKey {
        case configOptions
        case models
        case modes
        case metadata = "_meta"
    }
}

public struct PromptResponse: Codable, Equatable, Sendable {
    public var stopReason: String
    public var usage: JSONValue?
    public var metadata: [String: JSONValue]?

    enum CodingKeys: String, CodingKey {
        case stopReason
        case usage
        case metadata = "_meta"
    }
}

public struct ACPNotification: Equatable, Sendable {
    public var method: String
    public var params: JSONValue?
}

public struct ACPAgentRequest: Equatable, Sendable {
    public var id: JSONRPCID
    public var method: String
    public var params: JSONValue?
}

public enum ACPClientError: Error, Equatable, Sendable {
    case closed
    case protocolViolation(String)
    case remote(JSONRPCError)
}
