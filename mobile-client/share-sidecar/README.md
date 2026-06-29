# Goose ACP Share Sidecar

This is a prototype desktop-side bridge for the cleanroom mobile client.

It accepts Iroh QUIC connections on `goose-acp-mobile/1`, then bridges length-prefixed ACP JSON-RPC frames to a local Goose WebSocket ACP endpoint.

```text
mobile IrohACPTransport
  -> Iroh QUIC bi stream
  -> goose-acp-share
  -> ws://127.0.0.1:3284/acp?token=...
  -> Goose ACP
```

## Run

Start Goose ACP locally:

```bash
GOOSE_SERVER__SECRET_KEY=dev-secret cargo run -p goose-cli -- serve --port 3284
```

Start the sidecar:

```bash
cargo run -- serve \
  --acp-ws-url ws://127.0.0.1:3284/acp \
  --acp-token dev-secret
```

The sidecar prints a `goosepair1.<payload>` QR token. The token contains the Iroh endpoint address plus a short-lived pairing secret. A client must prove possession of that secret on the pairing control stream before ACP frames are accepted.

You can also use the sidecar binary as a low-level Iroh probe:

```bash
cargo run -- probe \
  --pairing-token 'goosepair1....' \
  --json '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientInfo":{"name":"probe","version":"0.1.0"}}}'
```

## Wire Format

After opening an Iroh bidirectional stream, the client writes:

```text
0x02                          stream kind: pairing control
u32be length + JSON bytes      PairingRequest
u32be length + JSON bytes      PairingResponse

0x01                          stream kind: ACP JSON frame stream, after accepted pairing
u32be length + JSON bytes      repeated JSON-RPC messages
```

The sidecar relays each JSON frame to the local ACP WebSocket as text and sends WebSocket text/data messages back as length-prefixed frames.

`PairingRequest` matches the Swift `PairingHandshake` payload: desktop nonce, mobile nonce, mobile device ID, mobile public key, and sorted requested capabilities joined with newlines, signed with HMAC-SHA256 over the QR secret.
