import Foundation

#if canImport(CryptoKit)
import CryptoKit
#endif

#if canImport(Security)
import Security
#endif

public struct PairingRequest: Codable, Equatable, Sendable {
    public var mobileDeviceID: String
    public var mobilePublicKey: String
    public var requestedCapabilities: [String]
    public var nonce: String
    public var proof: String
}

public enum PairingHandshake {
    public static func makeRequest(
        token: PairingToken,
        mobileDeviceID: String,
        mobilePublicKey: String,
        requestedCapabilities: [String],
        now: Date = Date()
    ) throws -> PairingRequest {
        guard token.expiresAt > now else {
            throw PairingError.expired
        }

        let nonce = Base64URL.encode(try randomBytes(count: 24))
        let canonicalCapabilities = requestedCapabilities.sorted()
        let payload = proofPayload(
            desktopNonce: token.pairingNonce,
            mobileNonce: nonce,
            mobileDeviceID: mobileDeviceID,
            mobilePublicKey: mobilePublicKey,
            requestedCapabilities: canonicalCapabilities
        )
        let proof = try sign(payload: payload, secret: token.pairingSecret)

        return PairingRequest(
            mobileDeviceID: mobileDeviceID,
            mobilePublicKey: mobilePublicKey,
            requestedCapabilities: canonicalCapabilities,
            nonce: nonce,
            proof: proof
        )
    }

    public static func verify(
        request: PairingRequest,
        token: PairingToken,
        now: Date = Date()
    ) throws -> Bool {
        guard token.expiresAt > now else {
            throw PairingError.expired
        }

        let payload = proofPayload(
            desktopNonce: token.pairingNonce,
            mobileNonce: request.nonce,
            mobileDeviceID: request.mobileDeviceID,
            mobilePublicKey: request.mobilePublicKey,
            requestedCapabilities: request.requestedCapabilities.sorted()
        )
        let expected = try sign(payload: payload, secret: token.pairingSecret)
        return expected == request.proof
    }

    private static func proofPayload(
        desktopNonce: String,
        mobileNonce: String,
        mobileDeviceID: String,
        mobilePublicKey: String,
        requestedCapabilities: [String]
    ) -> Data {
        let text = [
            desktopNonce,
            mobileNonce,
            mobileDeviceID,
            mobilePublicKey,
            requestedCapabilities.joined(separator: ","),
        ].joined(separator: "\n")
        return Data(text.utf8)
    }

    private static func sign(payload: Data, secret: String) throws -> String {
        #if canImport(CryptoKit)
        let secretData = try Base64URL.decode(secret)
        let key = SymmetricKey(data: secretData)
        let mac = HMAC<SHA256>.authenticationCode(for: payload, using: key)
        return Base64URL.encode(Data(mac))
        #else
        throw PairingError.cryptoUnavailable
        #endif
    }

    private static func randomBytes(count: Int) throws -> Data {
        #if canImport(Security)
        var bytes = [UInt8](repeating: 0, count: count)
        guard SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess else {
            throw PairingError.cryptoUnavailable
        }
        return Data(bytes)
        #else
        throw PairingError.cryptoUnavailable
        #endif
    }
}
