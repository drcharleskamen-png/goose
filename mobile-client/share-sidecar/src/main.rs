use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use clap::{Parser, Subcommand, ValueEnum};
use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, KeyInit, Mac};
use iroh::{Endpoint, EndpointAddr, SecretKey};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio_tungstenite::tungstenite::Message;

const ALPN: &[u8] = b"goose-acp-mobile/1";
const STREAM_ACP_JSON: u8 = 0x01;
const STREAM_PAIRING_CONTROL: u8 = 0x02;
const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
type HmacSha256 = Hmac<sha2::Sha256>;

#[derive(Debug, Parser)]
#[command(name = "goose-acp-share")]
#[command(about = "Prototype Iroh bridge for remote Goose ACP sessions")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Serve(ServeArgs),
    Probe(ProbeArgs),
    DecodeToken { token: String },
}

#[derive(Debug, Parser)]
struct ServeArgs {
    #[arg(long, default_value = "ws://127.0.0.1:3284/acp")]
    acp_ws_url: String,

    #[arg(long, env = "GOOSE_SERVER__SECRET_KEY")]
    acp_token: Option<String>,

    #[arg(long, default_value = "Mic's Mac")]
    desktop_name: String,

    #[arg(long, default_value = "15")]
    pairing_ttl_minutes: u64,

    #[arg(long)]
    key_path: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = RelayModeArg::Default)]
    relay: RelayModeArg,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum RelayModeArg {
    Default,
    Disabled,
}

#[derive(Debug, Parser)]
struct ProbeArgs {
    #[arg(long)]
    pairing_token: String,

    #[arg(long)]
    json: String,

    #[arg(long, default_value = "10")]
    connect_timeout_seconds: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "goose_acp_share=info,info".into()),
        )
        .init();

    match Cli::parse().command {
        Command::Serve(args) => serve(args).await,
        Command::Probe(args) => probe(args).await,
        Command::DecodeToken { token } => {
            let token = PairingToken::decode_qr(&token)?;
            println!("{}", serde_json::to_string_pretty(&token)?);
            Ok(())
        }
    }
}

async fn probe(args: ProbeArgs) -> Result<()> {
    let token = PairingToken::decode_qr(&args.pairing_token)?;
    let addr = decode_endpoint_addr(&token.desktop_endpoint)?;
    let mut builder = Endpoint::builder(iroh::endpoint::presets::Minimal)
        .secret_key(SecretKey::generate())
        .alpns(vec![ALPN.to_vec()]);
    builder = builder.relay_mode(relay_mode_from_endpoint_addr(&addr));
    let endpoint = builder.bind().await.context("bind probe endpoint")?;

    let timeout = Duration::from_secs(args.connect_timeout_seconds);
    if addr.relay_urls().next().is_some() {
        let _ = tokio::time::timeout(timeout, endpoint.online()).await;
    }

    let connection = tokio::time::timeout(timeout, endpoint.connect(addr, ALPN))
        .await
        .context("connect timed out")?
        .context("connect Iroh endpoint")?;
    let pairing_request = PairingRequest::for_token(
        &token,
        "goose-acp-share-probe".to_string(),
        endpoint.id().to_string(),
        token.capabilities.clone(),
    )?;
    let pairing_response =
        tokio::time::timeout(timeout, pair_connection(&connection, pairing_request))
            .await
            .context("pairing timed out")??;
    tracing::info!(
        "pairing accepted with capabilities: {}",
        pairing_response.capabilities.join(",")
    );

    let (mut send, mut recv) = connection.open_bi().await.context("open ACP stream")?;
    send.write_all(&[STREAM_ACP_JSON])
        .await
        .context("write stream kind")?;
    write_frame(&mut send, args.json.as_bytes()).await?;

    let response = tokio::time::timeout(timeout, read_frame(&mut recv))
        .await
        .context("read response timed out")??;
    match response {
        Some(bytes) => println!("{}", String::from_utf8_lossy(&bytes)),
        None => println!("stream closed without response"),
    }
    let _ = send.finish();
    endpoint.close().await;
    Ok(())
}

async fn serve(args: ServeArgs) -> Result<()> {
    let secret_key = match args.key_path.as_deref() {
        Some(path) => load_or_create_secret_key(path)?,
        None => SecretKey::generate(),
    };

    let mut builder = Endpoint::builder(iroh::endpoint::presets::Minimal)
        .secret_key(secret_key)
        .alpns(vec![ALPN.to_vec()]);
    if matches!(args.relay, RelayModeArg::Disabled) {
        builder = builder.relay_mode(iroh::endpoint::RelayMode::Disabled);
    }
    let endpoint = builder.bind().await.context("bind Iroh endpoint")?;
    if matches!(args.relay, RelayModeArg::Default) {
        let _ = tokio::time::timeout(Duration::from_secs(5), endpoint.online()).await;
    }

    let endpoint_addr = endpoint.addr();
    let token = PairingToken::new(
        endpoint.id().to_string(),
        encode_endpoint_addr(&endpoint_addr)?,
        endpoint_addr
            .relay_urls()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        args.desktop_name,
        Duration::from_secs(args.pairing_ttl_minutes * 60),
    )?;
    let pairing_gate = PairingGate::from_token(&token)?;

    println!("pairing_token={}", token.encode_qr()?);
    println!("desktop_id={}", endpoint.id());
    println!("listening_alpn={}", String::from_utf8_lossy(ALPN));
    println!("acp_ws_url={}", redact_token_url(&args.acp_ws_url));

    while let Some(incoming) = endpoint.accept().await {
        let acp_ws_url = acp_url_with_token(&args.acp_ws_url, args.acp_token.as_deref());
        let pairing_gate = pairing_gate.clone();
        tokio::spawn(async move {
            match incoming.await {
                Ok(connection) => {
                    let remote = connection.remote_id();
                    tracing::info!("accepted Iroh connection from {remote}");
                    if let Err(error) =
                        handle_connection(connection, acp_ws_url, Some(pairing_gate)).await
                    {
                        tracing::warn!("connection error: {error:#}");
                    }
                }
                Err(error) => tracing::warn!("incoming connection failed: {error}"),
            }
        });
    }

    Ok(())
}

async fn handle_connection(
    connection: iroh::endpoint::Connection,
    acp_ws_url: String,
    pairing_gate: Option<PairingGate>,
) -> Result<()> {
    let mut authorized = pairing_gate.is_none();

    loop {
        let (mut send, mut recv) = match connection.accept_bi().await {
            Ok(streams) => streams,
            Err(error) => {
                tracing::debug!("Iroh connection closed: {error}");
                return Ok(());
            }
        };

        let mut kind = [0u8; 1];
        recv.read_exact(&mut kind)
            .await
            .context("read stream kind")?;
        match kind[0] {
            STREAM_PAIRING_CONTROL => {
                let response = match pairing_gate.as_ref() {
                    Some(gate) => match accept_pairing_control(&mut recv, gate).await {
                        Ok(capabilities) => {
                            authorized = true;
                            PairingResponse::accepted(capabilities)
                        }
                        Err(error) => PairingResponse::rejected(error.to_string()),
                    },
                    None => {
                        authorized = true;
                        PairingResponse::accepted(Vec::new())
                    }
                };
                let accepted = response.accepted;
                let bytes = serde_json::to_vec(&response).context("serialize pairing response")?;
                write_frame(&mut send, &bytes).await?;
                let _ = send.finish();
                if !accepted {
                    bail!("pairing rejected");
                }
            }
            STREAM_ACP_JSON => {
                if !authorized {
                    let _ = send.finish();
                    bail!("ACP stream opened before pairing handshake");
                }

                let acp_ws_url = acp_ws_url.clone();
                tokio::spawn(async move {
                    if let Err(error) = bridge_acp_json_stream(send, recv, acp_ws_url).await {
                        tracing::warn!("ACP bridge stream error: {error:#}");
                    }
                });
            }
            other => {
                tracing::warn!("dropping unknown stream kind 0x{other:02x}");
                let _ = send.finish();
            }
        }
    }
}

async fn pair_connection(
    connection: &iroh::endpoint::Connection,
    request: PairingRequest,
) -> Result<PairingResponse> {
    let (mut send, mut recv) = connection.open_bi().await.context("open pairing stream")?;
    send.write_all(&[STREAM_PAIRING_CONTROL])
        .await
        .context("write pairing stream kind")?;
    let bytes = serde_json::to_vec(&request).context("serialize pairing request")?;
    write_frame(&mut send, &bytes).await?;

    let response = read_frame(&mut recv)
        .await?
        .context("pairing stream closed without response")?;
    let response: PairingResponse =
        serde_json::from_slice(&response).context("parse pairing response")?;
    let _ = send.finish();
    if !response.accepted {
        bail!(
            "pairing rejected: {}",
            response
                .message
                .as_deref()
                .unwrap_or("no rejection reason provided")
        );
    }
    Ok(response)
}

async fn accept_pairing_control(
    recv: &mut iroh::endpoint::RecvStream,
    gate: &PairingGate,
) -> Result<Vec<String>> {
    let bytes = read_frame(recv)
        .await?
        .context("pairing stream closed without request")?;
    let request: PairingRequest =
        serde_json::from_slice(&bytes).context("parse pairing request")?;
    gate.verify(&request)
}

async fn bridge_acp_json_stream(
    quic_send: iroh::endpoint::SendStream,
    quic_recv: iroh::endpoint::RecvStream,
    acp_ws_url: String,
) -> Result<()> {
    let (ws, _) = tokio_tungstenite::connect_async(&acp_ws_url)
        .await
        .with_context(|| {
            format!(
                "connect local ACP websocket {}",
                redact_token_url(&acp_ws_url)
            )
        })?;
    let (mut ws_write, mut ws_read) = ws.split();

    let iroh_to_ws = tokio::spawn(async move {
        let mut recv = quic_recv;
        while let Some(frame) = read_frame(&mut recv).await? {
            let text = String::from_utf8(frame).context("ACP frame was not UTF-8 JSON")?;
            ws_write.send(Message::Text(text.into())).await?;
        }
        let _ = ws_write.close().await;
        Ok::<_, anyhow::Error>(())
    });

    let ws_to_iroh = tokio::spawn(async move {
        let mut send = quic_send;
        while let Some(message) = ws_read.next().await {
            match message? {
                Message::Text(text) => write_frame(&mut send, text.as_bytes()).await?,
                Message::Binary(bytes) => write_frame(&mut send, &bytes).await?,
                Message::Close(_) => break,
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
            }
        }
        let _ = send.finish();
        Ok::<_, anyhow::Error>(())
    });

    let (left, right) = tokio::join!(iroh_to_ws, ws_to_iroh);
    left.context("Iroh to WebSocket task panicked")??;
    right.context("WebSocket to Iroh task panicked")??;
    Ok(())
}

async fn read_frame(recv: &mut iroh::endpoint::RecvStream) -> Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match recv.read_exact(&mut len).await {
        Ok(_) => {}
        Err(iroh::endpoint::ReadExactError::FinishedEarly(0)) => return Ok(None),
        Err(iroh::endpoint::ReadExactError::FinishedEarly(read)) => {
            bail!("stream ended in the middle of frame length after {read} bytes")
        }
        Err(error) => return Err(error).context("read frame length"),
    }
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME_BYTES {
        bail!("frame too large: {len} bytes");
    }
    let mut frame = vec![0; len];
    recv.read_exact(&mut frame)
        .await
        .context("read frame body")?;
    Ok(Some(frame))
}

async fn write_frame(send: &mut iroh::endpoint::SendStream, frame: &[u8]) -> Result<()> {
    if frame.len() > MAX_FRAME_BYTES {
        bail!("frame too large: {} bytes", frame.len());
    }
    send.write_all(&(frame.len() as u32).to_be_bytes())
        .await
        .context("write frame length")?;
    send.write_all(frame).await.context("write frame body")?;
    Ok(())
}

fn encode_endpoint_addr(addr: &EndpointAddr) -> Result<String> {
    let bytes = serde_json::to_vec(addr).context("serialize endpoint address")?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

fn decode_endpoint_addr(value: &str) -> Result<EndpointAddr> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .context("decode endpoint address")?;
    serde_json::from_slice(&bytes).context("parse endpoint address")
}

fn relay_mode_from_endpoint_addr(addr: &EndpointAddr) -> iroh::endpoint::RelayMode {
    let configs: Vec<_> = addr
        .relay_urls()
        .cloned()
        .map(|url| iroh::RelayConfig::new(url, None))
        .collect();
    if configs.is_empty() {
        iroh::endpoint::RelayMode::Disabled
    } else {
        iroh::endpoint::RelayMode::Custom(iroh::RelayMap::from_iter(configs))
    }
}

fn load_or_create_secret_key(path: &Path) -> Result<SecretKey> {
    if path.exists() {
        let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        let bytes: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| anyhow!("{} must contain a 32-byte Iroh secret key", path.display()))?;
        return Ok(SecretKey::from_bytes(&bytes));
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let key = SecretKey::generate();
    std::fs::write(path, key.to_bytes()).with_context(|| format!("write {}", path.display()))?;
    Ok(key)
}

fn acp_url_with_token(url: &str, token: Option<&str>) -> String {
    let Some(token) = token.filter(|token| !token.is_empty()) else {
        return url.to_string();
    };
    if url.contains("?") {
        format!("{url}&token={}", urlencoding(token))
    } else {
        format!("{url}?token={}", urlencoding(token))
    }
}

fn redact_token_url(url: &str) -> String {
    match url.split_once("token=") {
        Some((prefix, _)) => format!("{prefix}token=<redacted>"),
        None => url.to_string(),
    }
}

fn urlencoding(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    out
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingToken {
    version: u8,
    desktop_id: String,
    desktop_endpoint: String,
    relay_urls: Vec<String>,
    pairing_nonce: String,
    pairing_secret: String,
    expires_at: String,
    desktop_name: Option<String>,
    capabilities: Vec<String>,
}

impl PairingToken {
    const SCHEME: &'static str = "goosepair1";

    fn new(
        desktop_id: String,
        desktop_endpoint: String,
        relay_urls: Vec<String>,
        desktop_name: String,
        ttl: Duration,
    ) -> Result<Self> {
        Ok(Self {
            version: 1,
            desktop_id,
            desktop_endpoint,
            relay_urls,
            pairing_nonce: random_b64(24),
            pairing_secret: random_b64(32),
            expires_at: iso8601_after(ttl)?,
            desktop_name: Some(desktop_name),
            capabilities: vec![
                "sessions:list".to_string(),
                "sessions:load".to_string(),
                "sessions:prompt".to_string(),
                "sessions:cancel".to_string(),
            ],
        })
    }

    fn encode_qr(&self) -> Result<String> {
        let bytes = serde_json::to_vec(self).context("serialize pairing token")?;
        Ok(format!(
            "{}.{}",
            Self::SCHEME,
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
        ))
    }

    fn decode_qr(value: &str) -> Result<Self> {
        let payload = value
            .strip_prefix(&format!("{}.", Self::SCHEME))
            .context("invalid pairing token scheme")?;
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .context("decode pairing token")?;
        let token: Self = serde_json::from_slice(&bytes).context("parse pairing token")?;
        if token.version != 1 {
            bail!("unsupported pairing token version {}", token.version);
        }
        let _ = decode_endpoint_addr(&token.desktop_endpoint)?;
        Ok(token)
    }
}

#[derive(Clone, Debug)]
struct PairingGate {
    desktop_nonce: String,
    pairing_secret: String,
    expires_at: time::OffsetDateTime,
    capabilities: Vec<String>,
}

impl PairingGate {
    fn from_token(token: &PairingToken) -> Result<Self> {
        let _ = decode_b64url(&token.pairing_secret).context("decode pairing secret")?;
        let expires_at = time::OffsetDateTime::parse(
            &token.expires_at,
            &time::format_description::well_known::Rfc3339,
        )
        .context("parse pairing expiration")?;
        Ok(Self {
            desktop_nonce: token.pairing_nonce.clone(),
            pairing_secret: token.pairing_secret.clone(),
            expires_at,
            capabilities: token.capabilities.clone(),
        })
    }

    fn verify(&self, request: &PairingRequest) -> Result<Vec<String>> {
        if time::OffsetDateTime::now_utc() > self.expires_at {
            bail!("pairing token expired");
        }

        let mut requested_capabilities = request.requested_capabilities.clone();
        requested_capabilities.sort();
        for capability in &requested_capabilities {
            if !self.capabilities.contains(capability) {
                bail!("unsupported requested capability {capability}");
            }
        }

        let proof = pairing_proof(
            &self.pairing_secret,
            &self.desktop_nonce,
            &request.nonce,
            &request.mobile_device_id,
            &request.mobile_public_key,
            &requested_capabilities,
        )?;
        if proof != request.proof {
            bail!("invalid pairing proof");
        }
        Ok(requested_capabilities)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingRequest {
    mobile_device_id: String,
    mobile_public_key: String,
    requested_capabilities: Vec<String>,
    nonce: String,
    proof: String,
}

impl PairingRequest {
    fn for_token(
        token: &PairingToken,
        mobile_device_id: String,
        mobile_public_key: String,
        mut requested_capabilities: Vec<String>,
    ) -> Result<Self> {
        requested_capabilities.sort();
        let nonce = random_b64(24);
        let proof = pairing_proof(
            &token.pairing_secret,
            &token.pairing_nonce,
            &nonce,
            &mobile_device_id,
            &mobile_public_key,
            &requested_capabilities,
        )?;
        Ok(Self {
            mobile_device_id,
            mobile_public_key,
            requested_capabilities,
            nonce,
            proof,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingResponse {
    accepted: bool,
    capabilities: Vec<String>,
    message: Option<String>,
}

impl PairingResponse {
    fn accepted(capabilities: Vec<String>) -> Self {
        Self {
            accepted: true,
            capabilities,
            message: None,
        }
    }

    fn rejected(message: String) -> Self {
        Self {
            accepted: false,
            capabilities: Vec::new(),
            message: Some(message),
        }
    }
}

fn pairing_proof(
    secret: &str,
    desktop_nonce: &str,
    mobile_nonce: &str,
    mobile_device_id: &str,
    mobile_public_key: &str,
    requested_capabilities: &[String],
) -> Result<String> {
    let secret = decode_b64url(secret).context("decode pairing secret")?;
    let payload = [
        desktop_nonce,
        mobile_nonce,
        mobile_device_id,
        mobile_public_key,
        &requested_capabilities.join(","),
    ]
    .join("\n");
    let mut mac = HmacSha256::new_from_slice(&secret).context("create pairing HMAC")?;
    mac.update(payload.as_bytes());
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}

fn decode_b64url(value: &str) -> Result<Vec<u8>> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .context("decode base64url")
}

fn random_b64(len: usize) -> String {
    let mut bytes = vec![0; len];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn iso8601_after(ttl: Duration) -> Result<String> {
    let expires = SystemTime::now()
        .checked_add(ttl)
        .context("pairing expiration overflow")?
        .duration_since(UNIX_EPOCH)
        .context("pairing expiration before epoch")?
        .as_secs();
    Ok(format!("{}Z", humantime_seconds(expires)?))
}

fn humantime_seconds(epoch_seconds: u64) -> Result<String> {
    let datetime = time::OffsetDateTime::from_unix_timestamp(epoch_seconds as i64)
        .context("format unix timestamp")?;
    Ok(datetime
        .format(&time::format_description::well_known::Rfc3339)
        .context("format RFC3339 timestamp")?
        .trim_end_matches('Z')
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    #[test]
    fn token_round_trips_and_validates_endpoint() {
        let key = SecretKey::generate();
        let addr = EndpointAddr::new(key.public());
        let token = PairingToken::new(
            "desktop".to_string(),
            encode_endpoint_addr(&addr).unwrap(),
            vec![],
            "desktop".to_string(),
            Duration::from_secs(60),
        )
        .unwrap();

        let qr = token.encode_qr().unwrap();
        let decoded = PairingToken::decode_qr(&qr).unwrap();
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.desktop_id, "desktop");
        assert!(decoded
            .capabilities
            .contains(&"sessions:prompt".to_string()));
    }

    #[test]
    fn pairing_gate_rejects_bad_proof() {
        let key = SecretKey::generate();
        let addr = EndpointAddr::new(key.public());
        let token = PairingToken::new(
            "desktop".to_string(),
            encode_endpoint_addr(&addr).unwrap(),
            vec![],
            "desktop".to_string(),
            Duration::from_secs(60),
        )
        .unwrap();
        let gate = PairingGate::from_token(&token).unwrap();
        let mut request = PairingRequest::for_token(
            &token,
            "phone".to_string(),
            "phone-public-key".to_string(),
            token.capabilities.clone(),
        )
        .unwrap();
        request.proof = "bad-proof".to_string();

        let error = gate.verify(&request).unwrap_err().to_string();
        assert!(error.contains("invalid pairing proof"));
    }

    #[test]
    fn acp_url_appends_token() {
        assert_eq!(
            acp_url_with_token("ws://127.0.0.1:3284/acp", Some("a b")),
            "ws://127.0.0.1:3284/acp?token=a%20b"
        );
        assert_eq!(
            acp_url_with_token("ws://127.0.0.1:3284/acp?x=1", Some("secret")),
            "ws://127.0.0.1:3284/acp?x=1&token=secret"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn iroh_bridge_relays_framed_json_to_local_websocket() {
        let ws_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ws_addr = ws_listener.local_addr().unwrap();
        let ws_task = tokio::spawn(async move {
            let (stream, _) = ws_listener.accept().await.unwrap();
            let mut ws = accept_async(stream).await.unwrap();
            let message = ws.next().await.unwrap().unwrap();
            let text = match message {
                Message::Text(text) => text,
                other => panic!("expected text websocket message, got {other:?}"),
            };
            assert!(text.contains(r#""method":"initialize""#));
            ws.send(Message::Text(
                r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1}}"#.into(),
            ))
            .await
            .unwrap();
        });

        let server_endpoint = Endpoint::builder(iroh::endpoint::presets::Minimal)
            .secret_key(SecretKey::generate())
            .alpns(vec![ALPN.to_vec()])
            .relay_mode(iroh::endpoint::RelayMode::Disabled)
            .clear_ip_transports()
            .bind_addr((std::net::Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .bind()
            .await
            .unwrap();
        let server_addr = server_endpoint.addr();
        assert!(
            server_addr.ip_addrs().next().is_some(),
            "test server endpoint must advertise a direct address"
        );
        let token = PairingToken::new(
            server_endpoint.id().to_string(),
            encode_endpoint_addr(&server_addr).unwrap(),
            vec![],
            "desktop".to_string(),
            Duration::from_secs(60),
        )
        .unwrap();
        let pairing_gate = PairingGate::from_token(&token).unwrap();
        let server_task = {
            let endpoint = server_endpoint.clone();
            tokio::spawn(async move {
                let incoming = endpoint.accept().await.unwrap();
                let connection = incoming.await.unwrap();
                handle_connection(connection, format!("ws://{ws_addr}"), Some(pairing_gate))
                    .await
                    .unwrap();
            })
        };

        let client_endpoint = Endpoint::builder(iroh::endpoint::presets::Minimal)
            .secret_key(SecretKey::generate())
            .alpns(vec![ALPN.to_vec()])
            .relay_mode(iroh::endpoint::RelayMode::Disabled)
            .clear_ip_transports()
            .bind_addr((std::net::Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .bind()
            .await
            .unwrap();
        let connection = tokio::time::timeout(
            Duration::from_secs(5),
            client_endpoint.connect(server_addr, ALPN),
        )
        .await
        .expect("Iroh connect timed out")
        .expect("Iroh connect failed");
        let pairing_request = PairingRequest::for_token(
            &token,
            "phone".to_string(),
            client_endpoint.id().to_string(),
            token.capabilities.clone(),
        )
        .unwrap();
        let pairing_response = tokio::time::timeout(
            Duration::from_secs(5),
            pair_connection(&connection, pairing_request),
        )
        .await
        .expect("timed out waiting for pairing response")
        .unwrap();
        assert!(pairing_response.accepted);

        let (mut send, mut recv) = connection.open_bi().await.unwrap();
        send.write_all(&[STREAM_ACP_JSON]).await.unwrap();
        write_frame(
            &mut send,
            br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}"#,
        )
        .await
        .unwrap();

        let response = tokio::time::timeout(Duration::from_secs(5), read_frame(&mut recv))
            .await
            .expect("timed out waiting for framed ACP response")
            .unwrap()
            .unwrap();
        assert_eq!(
            String::from_utf8(response).unwrap(),
            r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1}}"#
        );

        let _ = send.finish();
        client_endpoint.close().await;
        server_endpoint.close().await;
        server_task.abort();
        ws_task.await.unwrap();
    }
}
