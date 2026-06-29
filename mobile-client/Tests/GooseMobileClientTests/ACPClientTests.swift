import Foundation
import XCTest
@testable import GooseMobileClient

final class ACPClientTests: XCTestCase {
    func testInitializeAndListSessions() async throws {
        let transport = MockACPTransport()
        let client = ACPClient(transport: transport)

        let initialize = try await client.initialize()
        XCTAssertEqual(initialize.protocolVersion, 1)
        XCTAssertEqual(initialize.agentInfo?.name, "goose")

        let sessions = try await client.listSessions(query: "work")
        XCTAssertEqual(sessions.sessions.map(\.sessionID), ["session-1"])

        let sent = await transport.sentMessages
        let listRequest = try XCTUnwrap(sent.last)
        XCTAssertEqual(listRequest["method"]?.stringValue, ACPMethod.listSessions)
        XCTAssertEqual(listRequest["params"]?["_meta"]?["query"]?.stringValue, "work")
    }

    func testPromptUsesLowercaseTextContentBlock() async throws {
        let transport = MockACPTransport()
        let client = ACPClient(transport: transport)

        _ = try await client.initialize()
        let response = try await client.prompt(sessionID: "session-1", text: "continue")

        XCTAssertEqual(response.stopReason, "end_turn")

        let sent = await transport.sentMessages
        let promptRequest = try XCTUnwrap(sent.last)
        let content = promptRequest["params"]?["prompt"]?.arrayValue?.first
        XCTAssertEqual(content?["type"]?.stringValue, "text")
        XCTAssertEqual(content?["text"]?.stringValue, "continue")
    }
}

actor MockACPTransport: ACPTransport {
    private(set) var sentMessages: [JSONValue] = []
    private var incoming: [JSONValue] = []
    private var waiter: CheckedContinuation<JSONValue?, Error>?

    func send(_ message: JSONValue) async throws {
        sentMessages.append(message)
        guard case let .request(request) = try JSONRPCIncomingMessage.decode(message),
              let id = request.id
        else {
            return
        }

        let result: JSONValue
        switch request.method {
        case ACPMethod.initialize:
            result = .object([
                "protocolVersion": .number(1),
                "agentInfo": .object([
                    "name": .string("goose"),
                    "version": .string("test"),
                ]),
            ])
        case ACPMethod.listSessions:
            result = .object([
                "sessions": .array([
                    .object([
                        "sessionId": .string("session-1"),
                        "cwd": .string("/tmp/project"),
                        "title": .string("Test session"),
                        "updatedAt": .string("2026-06-27T00:00:00Z"),
                    ]),
                ]),
            ])
        case ACPMethod.prompt:
            result = .object(["stopReason": .string("end_turn")])
        default:
            result = .object([:])
        }

        enqueue(.object([
            "jsonrpc": .string("2.0"),
            "id": try JSONValue.encoded(id),
            "result": result,
        ]))
    }

    func receive() async throws -> JSONValue? {
        if !incoming.isEmpty {
            return incoming.removeFirst()
        }

        return try await withCheckedThrowingContinuation { continuation in
            waiter = continuation
        }
    }

    func close() async {
        waiter?.resume(returning: nil)
        waiter = nil
    }

    private func enqueue(_ message: JSONValue) {
        if let waiter {
            self.waiter = nil
            waiter.resume(returning: message)
        } else {
            incoming.append(message)
        }
    }
}
