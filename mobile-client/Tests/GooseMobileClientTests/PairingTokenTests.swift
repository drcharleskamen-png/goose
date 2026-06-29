import Foundation
import XCTest
@testable import GooseMobileClient

final class PairingTokenTests: XCTestCase {
    func testQRCodeRoundTrip() throws {
        let token = sampleToken()
        let encoded = try token.encodeForQRCode()
        let decoded = try PairingToken.decodeQRCode(encoded)
        XCTAssertEqual(decoded, token)
    }

    func testRejectsWrongScheme() throws {
        XCTAssertThrowsError(try PairingToken.decodeQRCode("notgoose.payload")) { error in
            XCTAssertEqual(error as? PairingError, .invalidScheme)
        }
    }

    func testPairingProofVerifies() throws {
        let token = sampleToken()
        let request = try PairingHandshake.makeRequest(
            token: token,
            mobileDeviceID: "iphone-1",
            mobilePublicKey: "pubkey",
            requestedCapabilities: ["sessions:prompt", "sessions:list"]
        )

        XCTAssertTrue(try PairingHandshake.verify(request: request, token: token))

        let tampered = PairingRequest(
            mobileDeviceID: "iphone-2",
            mobilePublicKey: request.mobilePublicKey,
            requestedCapabilities: request.requestedCapabilities,
            nonce: request.nonce,
            proof: request.proof
        )
        XCTAssertFalse(try PairingHandshake.verify(request: tampered, token: token))
    }

    func testPairingResponseDecodes() throws {
        let data = Data(#"{"accepted":true,"capabilities":["sessions:list"],"message":null}"#.utf8)
        let response = try JSONDecoder().decode(PairingResponse.self, from: data)

        XCTAssertEqual(response, PairingResponse(accepted: true, capabilities: ["sessions:list"]))
    }

    private func sampleToken() -> PairingToken {
        PairingToken(
            desktopID: "desktop-1",
            desktopEndpoint: "endpoint-addr",
            relayURLs: ["https://relay.example"],
            pairingNonce: "desktop-nonce",
            pairingSecret: "c2VjcmV0LXNlY3JldC1zZWNyZXQ",
            expiresAt: Date(timeIntervalSince1970: 4_102_444_800),
            desktopName: "Mic's Mac"
        )
    }
}
