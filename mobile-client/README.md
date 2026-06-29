# Goose Mobile Client Prototype

This is a cleanroom mobile-client prototype for remote Goose sessions over ACP.

The current pass is Swift-first and deliberately small:

- ACP JSON-RPC client for `initialize`, `session/list`, `session/load`, `session/prompt`, and `session/cancel`
- WebSocket transport for direct local/dev testing against `goose serve`
- Length-prefixed JSON frame transport for the Iroh sidecar path
- QR pairing token model, one-time HMAC pairing proof, and typed pairing response
- Transport abstraction so an Iroh relay tunnel can replace direct WebSocket without changing ACP client code

The intended product shape is:

```text
SwiftUI app
  -> GooseMobileClient ACPClient
  -> ACPTransport
     -> WebSocketACPTransport for local/dev
     -> FramedACPTransport + Iroh byte stream later
  -> desktop goose-acp-share sidecar
  -> local Goose /acp endpoint
```

## Try the Direct ACP Prototype

Start Goose ACP locally with a secret:

```bash
GOOSE_SERVER__SECRET_KEY=dev-secret cargo run -p goose-cli -- serve --port 3284
```

Then, from this directory:

```bash
swift run goose-mobile-demo \
  --url ws://127.0.0.1:3284/acp \
  --token dev-secret \
  list
```

To nudge an existing session:

```bash
swift run goose-mobile-demo \
  --url ws://127.0.0.1:3284/acp \
  --token dev-secret \
  prompt <session-id> "continue from here"
```

## Iroh Hook Point

`ACPTransport` is the boundary. The Iroh-backed implementation should preserve the same API and hide whether bytes are carried through direct QUIC, a relay, or a local loopback proxy.

The desktop sidecar speaks length-prefixed JSON frames. Swift already has `FramedACPTransport`, so the eventual Iroh code only needs to implement `ACPByteTransport`.

The Iroh transport should live beside `WebSocketACPTransport` as `IrohACPTransport`, using the sidecar protocol:

```text
ALPN: goose-acp-mobile/1
stream 0x01: ACP JSON frame stream
stream 0x02: pairing control handshake
```

Pairing is intentionally separate from ACP. The QR token is a short-lived bearer secret only for bootstrapping trust; after pairing, the desktop should store the mobile device public key and enforce revocation/capabilities there.

The sidecar currently requires stream `0x02` to send a `PairingRequest` before stream `0x01` can bridge ACP. The proof payload matches `PairingHandshake`: desktop nonce, mobile nonce, mobile device ID, mobile public key, and sorted capabilities joined with newlines, signed with HMAC-SHA256 over the QR secret.
