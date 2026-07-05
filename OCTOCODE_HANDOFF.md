# OctoCode — Fable Mission Brief

You are building **OctoCode** — a fork of Goose (Apache-2.0) rebuilt into the most powerful agentic coding + automation CLI, optimized for GLM 5.2, MiniMax M3, DeepSeek V4/R1. Anthropic supported but deprioritized.

## Read first

1. **`OCTOCODE_SPEC.md`** (this repo) — the canonical spec: architecture, power features, maximal toolkit, token economy, Anthropic policy, legal, phasing, scope discipline, what's already done, open questions. Treat it as the contract.
2. **`CLAUDE.md`** + **`AGENTS.md`** (this repo) — Goose's contributor guidance.
3. Goose is **Rust** (Cargo workspace). Code lives in `crates/`. Provider + router code is your first modification target.

## Current state (do not redo)

Goose 1.41.0 installed. Forked at github.com/drcharleskamen-png/goose, cloned to `~/Desktop/goose`. Providers verified working end-to-end:
- GLM-5.2 (default) via Z.AI Anthropic-compat
- MiniMax-M3 via openai-compat
- DeepSeek V4 (deepseek-chat) via openai-compat
- DeepSeek R1 (deepseek-reasoner) via openai-compat

Shell switchers in `~/.zshrc`: `goose`=GLM, `gmm`=MiniMax, `gds`=V4, `gdsr`=R1, `gcc`=Claude. Keys in `~/.env`. Config in `~/.config/goose/config.yaml`.

## Hard rules (non-negotiable)

1. **No leaked source.** Do not use, reference, or "take inspiration from" leaked Claude Code source. Build on open protocols (MCP + ACP + OpenAI-compat) from observed behavior + public docs only. See §6 of spec.
2. **Apache-2.0 hygiene.** Preserve Goose LICENSE + attribution. Add Charles's copyright. Rebrand goose→octocode cleanly when ready.
3. **Token economy is priority.** Every feature justifies its token cost. Part 4 of spec is load-bearing from v1.0.
4. **Scope discipline.** Ship v1.0 spine (§7) before maximal toolkit. Cut §3.21 (specialized tools) to post-2.0. See §8.
5. **Plugin architecture from day one.** Core ships as plugins. Marketplace foundation in v1.0.
6. **CLI-first.** TUI on top. GUI later. Mobile much later.
7. **Security defaults conservative.** Sandbox untrusted tools/computer-control. Secret-scan. Destructive actions confirm. Never store 2FA secrets. WASM sandbox for plugins.
8. **Escalate open questions** (§10 of spec) — don't guess on licensing, trademark, repo name, first business profile.

## First task (concrete, do this before anything else)

1. `cd ~/Desktop/goose && cargo build` — verify toolchain works.
2. Read `CLAUDE.md` + `AGENTS.md`. Map `crates/` structure. Find provider + router code (`crates/goose/src/providers/` likely).
3. Produce a 1-page **architecture map**: where does provider abstraction live, where does routing live, where does token accounting happen (if anywhere yet), where do hooks/extensions plug in, where is the system prompt constructed, where is the tool-call transport. This map drives every modification.
4. Get Charles's sign-off on the map.
5. Then implement v1.0 spine in this order: provider abstraction generalization → router with per-task pinning → tool bridge parity → **token economy layer (Part 4)** → auto-CLAUDE.md → plugin/SDK foundation → Chrome control core.

## Pushback already incorporated into spec (do not re-litigate)

- MoA default OFF (not everywhere).
- Speculative branches read-only or worktree-isolated only.
- Self-improvement loop must dedup + conflict-detect + prune + expire.
- Auto-CLAUDE.md metric = real task success, not weak self-test.
- Tool list lazy-loads by task (token economy).
- Self-host priority = opt-in, not global gate.
- Single sandbox for v1 (Docker).
- Multi-surface sequenced, not simultaneous.
- "Beats Manus" deferred — match core browser first.

## What Charles cares about most

Token efficiency (target ≥60% reduction vs stock Claude Code), real-task quality on his businesses (Live Now Longevity peptide clinic, PepMaxx Labs, Octopuss AI), autonomous operation (he runs Openclaw swarm), and compounding memory (Obsidian + auto-CLAUDE.md + self-improvement). Build for those, not for benchmarks.

## Communicate

Status goes in commits + a running `OCTOCODE_LOG.md` (you create). Every PR-sized chunk: what changed, why, token-cost impact, what's next. Escalate blockers + open questions immediately — do not improvise past architectural decisions.
