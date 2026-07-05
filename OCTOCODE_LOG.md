# OctoCode Log

## 2026-07-03 — Session 1: architecture map + toolchain check

**What changed**
- Produced `OCTOCODE_ARCHMAP.md` — 1-page map of provider abstraction, routing insertion point, token accounting, prompt construction, tool transport, plugin points. Awaiting Charles's sign-off before v1.0 spine implementation.
- Created this log.

**Findings**
- Goose already has: Provider trait with streaming + cache-token usage extraction, 30+ declarative providers (DeepSeek listed), Anthropic prompt-caching (system/tools/messages), canonical model pricing, per-session cost accumulation, auto-compaction at 80%, parallel tool execution, hooks (11 events), plugins/skills, scheduler, Telegram gateway, local inference (llama.cpp/MLX). v1.0 is more extension than rewrite.
- Z.AI /v1 bug root cause: `openai_compatible.rs:121` appends `chat/completions` to a configurable `completions_prefix` — a declarative GLM definition with the right path is the clean fix.
- No router exists — clean greenfield at `model_config.rs` + subagent spawn.

**Token-cost impact**
- None yet (no runtime changes). Map identifies existing cache-token plumbing (`Usage.cache_read/write_input_tokens`) as the base for Part 4 accounting.

**Blockers (escalated)**
1. Disk full: 537MB free of 228GB (97%). Debug build died with `No space left on device`. Needs ~15-25GB free. Candidates: `~/.ollama` 18G (models), `~/Library/Caches` 7.6G, `~/.hermes` 6G, `~/.npm` 4G, `~/.cache` 3.3G. Not deleting anything without approval.
2. v8 build script (dep `v8-goose-145.0.2`) downloads prebuilt v8 via Python 3.14 urllib → `SSL: CERTIFICATE_VERIFY_FAILED`. Fix on retry: run `/Applications/Python 3.14/Install Certificates.command` (or set `SSL_CERT_FILE` to a certifi bundle).

**Next step**
- Charles: free disk + sign off on map (or amend). Then implement spine item 1: GLM/MiniMax/DeepSeek first-class provider definitions + canonical pricing.

## 2026-07-04 — Session 1 (cont): blockers cleared, spine item 1

**Approvals**
- Charles approved architecture map + cache cleanup (npm/.cache/Library caches, ~15GB freed).

**Blockers resolved**
1. Disk: 537MB → ~9.7GB free after approved cache cleanup.
2. v8 SSL: ran `Install Certificates.command` for Python 3.14 — v8 download now works.
3. Build ICE root-caused: `librusty_v8.a` truncated at 112,427,008 bytes (disk filled mid-download on first build); rustc archive reader panicked with exactly that slice length. Deleted `target/debug/gn_out` + v8-goose artifacts; rebuild in progress.

**Spine item 1 findings (big scope reduction)**
- Upstream already ships first-class defs: `zai.json` (glm-5.2, 1M ctx, anthropic engine, fast_model glm-4.5-air), `custom_deepseek` (deepseek-chat + deepseek-reasoner). The manual OPENAI_HOST shell hacks are obsolete — `GOOSE_PROVIDER=zai` + `ZHIPU_API_KEY` is the supported path.
- Canonical pricing already present: zhipuai/glm-5.2 ($1.4/$4.4, cache_read $0.26), minimax/MiniMax-M3 ($0.3/$1.2, cache_read $0.06, 1M ctx), deepseek/deepseek-chat + reasoner ($0.14/$0.28, cache_read $0.0028, 1M ctx). Cost dashboard foundation exists.
- Only real gap: minimax.json lacked M3. **Changed**: added `MiniMax-M3` (context_limit 1000000) to `crates/goose-providers/src/declarative/definitions/minimax.json`. Bundled-provider validation test covers JSON shape.

**Token-cost impact**
- zai anthropic engine gets Anthropic-style prompt caching (system/tools/last-2-messages cache_control) for free → GLM cache_read at $0.26 vs $1.4 input.

**Next step**
- Verify build + run `cargo test -p goose-providers` for def validation. Then spine item 2: router module + per-task pinning.

**Build saga — root causes (for the record)**
1. Disk-full build #1 left a truncated `librusty_v8.a` (112,427,008 bytes vs 112,939,632 expected) → deterministic rustc ICE. Fixed by deleting `target/debug/gn_out` + v8-goose artifacts.
2. Same disk-full event corrupted the compiled `.rmeta` of the vendored `v8` shim → 156 phantom compile errors in `deno_core` (E0061/E0432 storms). Cargo trusted the fingerprint; `.crate` archives verified pristine (checksums match lock), upstream CI green on identical commit. Fixed by `cargo clean -p v8 -p serde_v8 -p deno_core -p deno_ops -p deno_error`.
3. Lesson: macOS Rust builds have zero upstream CI coverage for the full workspace (ubuntu tests workspace; macOS builds only `-p goose-cli`). Full workspace debug build needs >18GB free. Standard dev flow on this machine: `cargo build -p goose-cli` with `CARGO_INCREMENTAL=0` until disk situation improves.

**Verified (2026-07-04)**
- `cargo build -p goose-cli` (CARGO_INCREMENTAL=0, CARGO_PROFILE_DEV_DEBUG=0): Finished in 8m04s. Binary runs (`goose 1.41.0`).
- `cargo test -p goose-providers declarative`: 10 passed, 0 failed — covers the minimax.json change via `all_bundled_providers_are_valid`.
- Committed `da52c538b feat(providers): add MiniMax-M3 to MiniMax declarative provider` (local main, not pushed).
- Spine item 1 complete. Config migration for Charles (replaces ~/.zshrc hacks): `GOOSE_PROVIDER=zai` + `ZHIPU_API_KEY` for GLM-5.2; `custom_deepseek` + `DEEPSEEK_API_KEY` for V4/R1; `minimax` + `MINIMAX_API_KEY` for M3.

**Next step**
- Spine item 2: router module (`crates/goose/src/router/`) with per-task pinning + `/model` override, generalizing the `complete_fast()` pattern in `model_config.rs`.

## 2026-07-04 — Session 1 (cont): provider config migration

**What changed**
- `~/.zshrc` (backup: `~/.zshrc.bak-20260704`): switchers rewritten from host-override hacks to native provider defs — goose→`zai`/glm-5.2, gmm→`minimax`/MiniMax-M3, gds/gdsr→`custom_deepseek`, gcc unchanged. Added `ZHIPU_API_KEY` mirror of `GLM_API_KEY` (zai def reads ZHIPU_API_KEY).
- `~/.config/goose/config.yaml` (backup: `config.yaml.bak-20260704`): added `zai` provider entry, `active_provider: openai` → `zai`. Fixes bare `goose` binary silently pointing glm-5.2 at api.openai.com.

**Verified** (live runs, session DB provider_name): zai glm-5.2 ✓, minimax MiniMax-M3 ✓ (brew binary accepts M3 without static def entry), custom_deepseek deepseek-chat ✓ + deepseek-reasoner ✓, new `goose()` function ✓, bare `command goose` via config default ✓.

**Token-cost impact**
- Sessions now attributed to correct canonical providers → `accumulated_cost` uses real pricing (zhipuai/glm-5.2 etc.) instead of misattributing GLM traffic to anthropic/openai. zai route keeps Anthropic-style prompt caching.

**Caveat**
- Desktop app (GUI launch) doesn't source ~/.zshrc — no ZHIPU_API_KEY env. If desktop use needed on zai, one-time `goose configure` to store the key in keyring.

## 2026-07-04 — Session 1 (cont): spine item 2 — routing

**Recon result: most of the routing matrix already exists upstream**
- Subagent pinning: `GOOSE_SUBAGENT_PROVIDER`/`GOOSE_SUBAGENT_MODEL` + per-`delegate()` params + recipe settings (summon.rs:1648).
- Planner pinning: `GOOSE_PLANNER_PROVIDER`/`GOOSE_PLANNER_MODEL`.
- `/model <name>`: same-provider switch existed.
- Old `GOOSE_LEAD_MODEL` (blog 2025-06) removed from source.

**What we added (fork commits)**
1. `GOOSE_FAST_PROVIDER` (`crates/goose/src/model_config.rs`): routes fast tasks (compaction, session naming, summarization, orchestrator summaries) to a different provider. Model = `GOOSE_FAST_MODEL` or the fast provider's declared fast model. Any failure → warn + fall back to session provider; a bad route can never break a session.
2. `/model <provider> <model>` (`crates/goose-cli/src/session/mod.rs`, `input.rs`): cross-provider session switch, persisted to session record.

**Verified live (fork binary)**
- zai/glm-5.2 session with `GOOSE_FAST_PROVIDER=custom_deepseek GOOSE_FAST_MODEL=deepseek-chat`: log shows "Routing fast task to custom_deepseek deepseek-chat", session named, no fallback.
- Broken DEEPSEEK key: auth error → warned → fell back to main model, session unaffected.
- `/model custom_deepseek deepseek-chat` in interactive session (expect-driven): "Session switched from 'glm-5.2' (zai) to 'deepseek-chat'…", session DB provider_name=custom_deepseek.

**Token-cost impact**
- Fast tasks are the highest-frequency background LLM calls. Routing them zai→deepseek-chat: $1.4→$0.14/MTok input, $4.4→$0.28/MTok output (~10x cheaper) on every compaction/naming/summarize call.

**Notes**
- Fork's first interactive run wrote `GOOSE_TELEMETRY_ENABLED: true` to config.yaml (consent prompt auto-answered by scripted test input). Flag for Charles: flip to false if unwanted; spec's telemetry-isolation work comes later.
- Recommended config once fork is daily driver: `GOOSE_FAST_PROVIDER: custom_deepseek`, `GOOSE_FAST_MODEL: deepseek-chat`. Do NOT set on brew binary (it lacks GOOSE_FAST_PROVIDER; GOOSE_FAST_MODEL alone would 404 against zai and spam fallback warnings).

**Next step**
- Spine item 3: tool bridge parity check across formats (openai/anthropic), then TOKEN ECONOMY layer (Part 4): budgets + cache-hit surfacing.

## 2026-07-05 — Session 1 (cont): fork = daily driver, GLM-optimized

**What changed**
- Release build of fork CLI (26m51s), installed to `~/.local/bin/goose-octocode` (stable path — `target/` gets cleaned). `target/release` intermediates deleted (+4.8GB disk).
- `~/.zshrc`: `OCTOCODE_BIN` resolution chain (installed copy → fresh build → brew); all switchers now run the fork binary.
- `~/.config/goose/config.yaml`: `GOOSE_TELEMETRY_ENABLED: false` (scripted test had accidentally consented), `GOOSE_FAST_PROVIDER: custom_deepseek`, `GOOSE_FAST_MODEL: deepseek-chat`.

**Verified live (fork release binary, fresh shells)**
- `goose` → zai glm-5.2 ✓; `gmm` → minimax MiniMax-M3 ✓; `gds` → custom_deepseek ✓.
- Fast routing engaged from config: three sessions logged "Routing fast task to custom_deepseek deepseek-chat".

**GLM-optimized daily state**: glm-5.2 @ 1M ctx via zai (prompt caching on, cache reads $0.26 vs $1.4/MTok), all fast tasks on deepseek-chat ($0.14/MTok), correct per-provider cost attribution, telemetry off.

**Caveats**
- Desktop app reads same config.yaml but is a separate binary without GOOSE_FAST_PROVIDER — its fast calls warn + fall back to session provider. Harmless; goes away when desktop is built from fork.
- Disk still chronic (~6GB free). Full rebuilds remain risky without a permanent 20GB+ cleanup.
