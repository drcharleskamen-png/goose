import Foundation

public protocol ACPByteTransport: Sendable {
    func send(_ data: Data) async throws
    func receive(upTo byteCount: Int) async throws -> Data?
    func close() async
}

public enum FramedACPTransportError: Error, Equatable, Sendable {
    case frameTooLarge(Int)
    case streamEndedMidFrame
}

public final class FramedACPTransport: ACPTransport, Sendable {
    public static let defaultMaxFrameBytes = 16 * 1024 * 1024

    private let byteTransport: any ACPByteTransport
    private let maxFrameBytes: Int

    public init(
        byteTransport: any ACPByteTransport,
        maxFrameBytes: Int = FramedACPTransport.defaultMaxFrameBytes
    ) {
        self.byteTransport = byteTransport
        self.maxFrameBytes = maxFrameBytes
    }

    public func send(_ message: JSONValue) async throws {
        let payload = try JSONEncoder().encode(message)
        guard payload.count <= maxFrameBytes else {
            throw FramedACPTransportError.frameTooLarge(payload.count)
        }

        var length = UInt32(payload.count).bigEndian
        var frame = Data(bytes: &length, count: MemoryLayout<UInt32>.size)
        frame.append(payload)
        try await byteTransport.send(frame)
    }

    public func receive() async throws -> JSONValue? {
        guard let lengthBytes = try await readExact(4) else {
            return nil
        }

        let length = lengthBytes.withUnsafeBytes { pointer -> UInt32 in
            pointer.load(as: UInt32.self).bigEndian
        }
        let count = Int(length)
        guard count <= maxFrameBytes else {
            throw FramedACPTransportError.frameTooLarge(count)
        }

        guard let payload = try await readExact(count) else {
            throw FramedACPTransportError.streamEndedMidFrame
        }
        return try JSONDecoder().decode(JSONValue.self, from: payload)
    }

    public func close() async {
        await byteTransport.close()
    }

    private func readExact(_ count: Int) async throws -> Data? {
        var buffer = Data()
        buffer.reserveCapacity(count)

        while buffer.count < count {
            guard let chunk = try await byteTransport.receive(upTo: count - buffer.count) else {
                if buffer.isEmpty {
                    return nil
                }
                throw FramedACPTransportError.streamEndedMidFrame
            }
            if chunk.isEmpty {
                continue
            }
            buffer.append(chunk)
        }

        return buffer
    }
}
