import Foundation

public protocol ACPTransport: Sendable {
    func send(_ message: JSONValue) async throws
    func receive() async throws -> JSONValue?
    func close() async
}
