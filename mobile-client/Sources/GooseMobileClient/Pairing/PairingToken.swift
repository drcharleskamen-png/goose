import Foundation

public struct PairingToken: Codable, Equatable, Sendable {
    public static let scheme = "goosepair1"

    public var version: Int
    public var desktopID: String
    public var desktopEndpoint: String
    public var relayURLs: [String]
    public var pairingNonce: String
    public var pairingSecret: String
    public var expiresAt: Date
    public var desktopName: String?
    public var capabilities: [String]

    public init(
        version: Int = 1,
        desktopID: String,
        desktopEndpoint: String,
        relayURLs: [String],
        pairingNonce: String,
        pairingSecret: String,
        expiresAt: Date,
        desktopName: String? = nil,
        capabilities: [String] = ["sessions:list", "sessions:load", "sessions:prompt", "sessions:cancel"]
    ) {
        self.version = version
        self.desktopID = desktopID
        self.desktopEndpoint = desktopEndpoint
        self.relayURLs = relayURLs
        self.pairingNonce = pairingNonce
        self.pairingSecret = pairingSecret
        self.expiresAt = expiresAt
        self.desktopName = desktopName
        self.capabilities = capabilities
    }

    public func encodeForQRCode() throws -> String {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        let data = try encoder.encode(self)
        return "\(Self.scheme).\(Base64URL.encode(data))"
    }

    public static func decodeQRCode(_ rawValue: String) throws -> PairingToken {
        let prefix = "\(scheme)."
        guard rawValue.hasPrefix(prefix) else {
            throw PairingError.invalidScheme
        }

        let encodedPayload = String(rawValue.dropFirst(prefix.count))
        let data = try Base64URL.decode(encodedPayload)
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let token = try decoder.decode(PairingToken.self, from: data)
        guard token.version == 1 else {
            throw PairingError.unsupportedVersion(token.version)
        }
        return token
    }
}

public enum PairingError: Error, Equatable, Sendable {
    case invalidScheme
    case invalidBase64URL
    case unsupportedVersion(Int)
    case expired
    case cryptoUnavailable
}
