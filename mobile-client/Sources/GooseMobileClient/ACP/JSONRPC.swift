import Foundation

public enum JSONRPCID: Codable, Equatable, Hashable, Sendable {
    case number(Int)
    case string(String)
    case null

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            self = .null
        } else if let value = try? container.decode(Int.self) {
            self = .number(value)
        } else if let value = try? container.decode(String.self) {
            self = .string(value)
        } else {
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "JSON-RPC id must be an integer, string, or null"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case let .number(value):
            try container.encode(value)
        case let .string(value):
            try container.encode(value)
        case .null:
            try container.encodeNil()
        }
    }

    var correlationKey: String {
        switch self {
        case let .number(value):
            return "n:\(value)"
        case let .string(value):
            return "s:\(value)"
        case .null:
            return "null"
        }
    }
}

public struct JSONRPCError: Codable, Error, Equatable, Sendable {
    public var code: Int
    public var message: String
    public var data: JSONValue?

    public init(code: Int, message: String, data: JSONValue? = nil) {
        self.code = code
        self.message = message
        self.data = data
    }
}

public struct JSONRPCRequest: Codable, Equatable, Sendable {
    public var jsonrpc: String
    public var id: JSONRPCID?
    public var method: String
    public var params: JSONValue?

    public init(id: JSONRPCID? = nil, method: String, params: JSONValue? = nil) {
        self.jsonrpc = "2.0"
        self.id = id
        self.method = method
        self.params = params
    }
}

public struct JSONRPCResponse: Codable, Equatable, Sendable {
    public var jsonrpc: String
    public var id: JSONRPCID
    public var result: JSONValue?
    public var error: JSONRPCError?

    public init(id: JSONRPCID, result: JSONValue? = nil, error: JSONRPCError? = nil) {
        self.jsonrpc = "2.0"
        self.id = id
        self.result = result
        self.error = error
    }
}

public enum JSONRPCIncomingMessage: Equatable, Sendable {
    case response(JSONRPCResponse)
    case request(JSONRPCRequest)
    case notification(JSONRPCRequest)

    public static func decode(_ value: JSONValue) throws -> JSONRPCIncomingMessage {
        let raw = try value.decode(RawJSONRPCMessage.self)
        if let method = raw.method {
            let request = JSONRPCRequest(id: raw.id, method: method, params: raw.params)
            return raw.id == nil ? .notification(request) : .request(request)
        }

        guard let id = raw.id else {
            throw ACPClientError.protocolViolation("Response is missing JSON-RPC id")
        }

        return .response(JSONRPCResponse(id: id, result: raw.result, error: raw.error))
    }
}

private struct RawJSONRPCMessage: Decodable {
    var id: JSONRPCID?
    var method: String?
    var params: JSONValue?
    var result: JSONValue?
    var error: JSONRPCError?
}
