# goose-sdk

The bindings layer for Goose. It houses the shared types used for both ACP and
SDK access, and exposes a cross-language version of the Goose API.

With `--features uniffi` the crate compiles to native bindings for Python and
Kotlin (namespace `goose` / `io.aaif.goose`). The UniFFI surface currently lets
callers construct declarative providers from JSON and stream provider
completions.

```bash
just python   # build bindings + run examples/uniffi/ping.py
just kotlin   # build bindings + run examples/uniffi/Ping.kt
```

Both print `pong: aaif.io`.
