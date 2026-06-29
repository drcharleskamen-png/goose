public struct PairingResponse: Codable, Equatable, Sendable {
    public var accepted: Bool
    public var capabilities: [String]
    public var message: String?

    public init(
        accepted: Bool,
        capabilities: [String],
        message: String? = nil
    ) {
        self.accepted = accepted
        self.capabilities = capabilities
        self.message = message
    }
}
