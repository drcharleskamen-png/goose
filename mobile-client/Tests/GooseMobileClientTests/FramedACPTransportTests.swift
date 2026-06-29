import Foundation
import XCTest
@testable import GooseMobileClient

final class FramedACPTransportTests: XCTestCase {
    func testSendWritesLengthPrefixedJSON() async throws {
        let bytes = MockByteTransport()
        let transport = FramedACPTransport(byteTransport: bytes)

        try await transport.send(.object(["method": .string("initialize")]))

        let sent = await bytes.sent
        XCTAssertEqual(sent.count, 1)
        let frame = try XCTUnwrap(sent.first)
        let length = frame.prefix(4).withUnsafeBytes { pointer -> UInt32 in
            pointer.load(as: UInt32.self).bigEndian
        }
        XCTAssertEqual(Int(length), frame.count - 4)

        let payload = frame.dropFirst(4)
        let decoded = try JSONDecoder().decode(JSONValue.self, from: payload)
        XCTAssertEqual(decoded["method"]?.stringValue, "initialize")
    }

    func testReceiveReadsFragmentedFrame() async throws {
        let bytes = MockByteTransport()
        let transport = FramedACPTransport(byteTransport: bytes)
        let payload = try JSONEncoder().encode(JSONValue.object(["ok": .bool(true)]))
        var length = UInt32(payload.count).bigEndian
        var frame = Data(bytes: &length, count: 4)
        frame.append(payload)

        await bytes.enqueue(frame.prefix(2))
        await bytes.enqueue(frame.dropFirst(2).prefix(5))
        await bytes.enqueue(frame.dropFirst(7))

        let decoded = try await transport.receive()
        XCTAssertEqual(decoded?["ok"], .bool(true))
    }
}

actor MockByteTransport: ACPByteTransport {
    private(set) var sent: [Data] = []
    private var inbound: [Data] = []

    func send(_ data: Data) async throws {
        sent.append(data)
    }

    func receive(upTo byteCount: Int) async throws -> Data? {
        guard !inbound.isEmpty else {
            return nil
        }

        let chunk = inbound.removeFirst()
        if chunk.count <= byteCount {
            return chunk
        }

        let prefix = chunk.prefix(byteCount)
        inbound.insert(chunk.dropFirst(byteCount), at: 0)
        return Data(prefix)
    }

    func close() async {}

    func enqueue<D: DataProtocol>(_ data: D) {
        inbound.append(Data(data))
    }
}
