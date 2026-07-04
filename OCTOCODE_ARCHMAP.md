# OctoCode Architecture Map (v1 — for sign-off)

One page. Where every v1.0 spine modification lands in the Goose codebase.

## 1. Provider abstraction — mostly exists, generalize don't rebuild

- **Trait**: `crates/goose-provider-types/src/base.rs:381` — `Provider` trait. `stream()` is primary; `complete()` delegates. Also `get_context_limit`, `retry_config`, `fetch_supported_models`, `map_to_canonical_model`.
- **Implementations**: `crates/goose-providers/src/` — `anthropic.rs`, `openai.rs`, `openai_compatible.rs`, `ollama.rs`, `databricks*.rs`, plus **30+ declarative providers** (`declarative.rs` + `definitions/` YAML — DeepSeek already listed) that map onto 3 engines (OpenAI/Anthropic/Ollama).
- **Registry/factory**: `crates/goose/src/providers/provider_registry.rs:86` — name → constructor. `*_def.rs` wrappers do env/config binding.
- **Wire formats + tool-call conversion**: `crates/goose-provider-types/src/formats/{openai,anthropic,ollama}.rs` — internal MCP `Tool` ↔ provider JSON. Usage (incl. cache read/write tokens) extracted here: `openai.rs:787`, `anthropic.rs:519`.
- **The Z.AI /v1 bug**: `crates/goose-providers/src/openai_compatible.rs:121` — `format!("{}chat/completions", completions_prefix)`. Prefix is configurable → declarative GLM def with correct path fixes it properly.
- **Prompt caching**: already implemented for Anthropic format — `formats/anthropic.rs:443` (system), `:430` (tools), `:323` (last-2-user-message cache_control). GLM via Anthropic-compat route inherits this for free.
- **Retry/backoff**: `goose-provider-types/src/retry.rs` — exp backoff + jitter, 429 retry-after extraction.

**v1.0 work**: first-class declarative defs for GLM-5.2 / MiniMax-M3 / DeepSeek V4/R1 with correct base URLs, context limits, pricing in canonical registry (`canonical/model.rs:38` `Pricing` + `estimate_cost`); per-provider thinking/reasoning params in `model.rs` `ModelConfig`.

## 2. Routing — does not exist yet; clean insertion point

- Today: single active provider/model from `crates/goose/src/config/providers.rs:65-87` (`GOOSE_PROVIDER`/`GOOSE_MODEL` env → config). Plus **fast model** (`model_config.rs:89-104`, `GOOSE_FAST_MODEL`) used for compaction/naming — an existing 2-tier precedent.
- Subagents: `agents/subagent_handler.rs:48` `run_subagent_task` takes own config → per-subagent model pinning hooks here.
- **v1.0 work**: new `router` module in `crates/goose`; decides model per task-class/filetype/override; plugs in at `model_config.rs` materialization + subagent spawn + `/model` in CLI `session/mod.rs:566` `handle_input`.

## 3. Token accounting — foundation exists

- Counting: `crates/goose/src/token_counter.rs` — tiktoken o200k + LRU cache (provider-specific tokenizers = v1.0 gap).
- Usage/cost: `session/session_manager.rs:61-96` — `accumulated_usage`, `accumulated_cost` per session (SQLite, schema v14); `agents/reply_parts.rs:558` `accumulate_cost()` via canonical pricing.
- Context mgmt: `crates/goose/src/context_mgmt/mod.rs` — auto-compact at 80% (`GOOSE_AUTO_COMPACT_THRESHOLD`), progressive tool-response stripping, background tool-call summarization using fast model.
- **v1.0 work (Part 4)**: budgets + burn-rate UI on top of existing Usage structs; cache-hit surfacing (cache token fields already parsed); tool-result cache; symbol-level reads; per-provider tokenizer registry.

## 4. Agent core / tool transport

- Loop: `crates/goose/src/agents/agent.rs:1522` `reply()` → `:1801` `reply_internal()` → `:1901` turn loop. Parallel tool exec already native: `:2189` `select_all`.
- Dispatch: `agent.rs:794` → `extension_manager.rs:1744` → MCP `call_tool`.
- Extensions: `agents/extension_manager.rs` (MCP servers + 11 platform builtins incl. `developer`, `computercontroller` base for Chrome control); bundled MCP servers in `crates/goose-mcp`.

## 5. System prompt construction

- `crates/goose/src/prompt_template.rs` (9 jinja templates in `src/prompts/`) + `agents/prompt_manager.rs:51` `SystemPromptBuilder` (extension instructions injected at `:118-127`).
- **v1.0 work**: per-provider prompt flavoring = template selection/params keyed off provider in `SystemPromptBuilder`.

## 6. Plugin points (load-bearing, all exist)

- Hooks: `crates/goose/src/hooks/mod.rs` — 11 events (PreToolUse … Stop), `.goose/hooks.json`.
- Plugins: `crates/goose/src/plugins/` — git-installed, carry skills/extensions.
- Skills: `crates/goose/src/skills/` (`~/.agents/skills`); slash commands: `src/slash_commands/`.
- Scheduler (cron recipes): `scheduler.rs`; Gateway (Telegram etc. → Openclaw-mode base): `src/gateway/`; local inference (self-host, llama.cpp/MLX): `crates/goose-local-inference`.
- Telemetry: `posthog.rs` (kill via `GOOSE_TELEMETRY_OFF`) — required for provider-neutral/offline policy.

## v1.0 spine → code mapping (build order)

| Spine item | Where | Nature |
|---|---|---|
| 1. Provider defs GLM/MiniMax/DeepSeek | `goose-providers/declarative` + canonical registry | extend |
| 2. Router + /model + per-task pinning | new `crates/goose/src/router/` + `model_config.rs` + subagent spawn | new |
| 3. Tool bridge parity | `goose-provider-types/formats/*` | verify/extend |
| 4. Token economy layer | `context_mgmt` + `reply_parts` + `session_manager` + new budget module | extend+new |
| 5. Auto-CLAUDE.md | new module + hooks + session data | new |
| 6. Plugin/SDK foundation | `plugins/` + `skills/` + extension manager | formalize |
| 7. Chrome control core | new `goose-mcp` extension (fork `computercontroller`) | new |
