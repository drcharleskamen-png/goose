# OctoCode — Canonical Specification

**OctoCode** = fork of [block/goose](https://github.com/block/goose) (Apache-2.0), rebuilt into the most powerful agentic coding CLI optimized for **GLM 5.2**, **MiniMax M3**, **DeepSeek V4 / R1**. Anthropic models supported but deprioritized.

Primary directive: build the most powerful agentic coding + automation harness ever shipped, with **token economy as a first-class priority**, on open protocols (MCP + ACP + OpenAI-compat), never on leaked source.

---

## 1. Architecture (foundational)

### A. Provider abstraction
`ModelProvider` interface: `stream()`, `complete()`, `count_tokens()`, `supports_tools()`, `supports_reasoning()`, `max_context()`, `cache_strategy()`, `cost_per_token()`.
- `GLMProvider`, `MiniMaxProvider`, `DeepSeekProvider` native first.
- `AnthropicProvider` native, supported, never default (see §5).

### B. Tool-calling bridge
- GLM/DeepSeek: native function-calling (JSON schema, tool_calls/tool roles).
- MiniMax M3: native tool-use + system tool defs; MCP tools passable directly.
- Anthropic: native tool_use.
- Identical tool SEMANTICS across providers. Aggressive parallel tool calls.

### C. Per-provider system prompt builder
Canonical harness instructions wrapped model-natively:
- GLM 5.2: structure + worked tool examples + bilingual.
- MiniMax M3: agentic framing (goals, success criteria, autonomy boundaries).
- DeepSeek V4: standard structured.
- DeepSeek R1: reasoning-friendly, show-work, escalate to thinking.

### D. Context window + smart compaction
~1M default. Auto-compact ~700k. Keep full contents in-context (reduces hallucination). Diff-aware retry. Smart compaction preserves exact tool I/O, summarizes prose.

### E. Prompt caching
Provider-specific (GLM context cache, MiniMax scheme, DeepSeek scheme, Anthropic breakpoints). Stable prefix cached (system prompt + tool schemas + CLAUDE.md/skills/constitution). Cache-hit UI + manual warming.

### F. Tokenizer + accounting
Provider tokenizers (all expose counts). Accurate compaction + cost.

### G. Reasoning / thinking
GLM thinking mode, MiniMax hybrid budget, DeepSeek R1 reasoning, Anthropic extended thinking. `/think 0-5`. Auto-escalate on retry.

### H. Subagents + parallelism
High concurrency defaults (cheap inference). Planner→executor→reviewer. Worktree isolation. `run_in_background`.

### I. MCP
All providers; MiniMax consumes MCP natively.

### J. Harness preservation
Full tool set (Read/Edit/Bash/Grep/Glob/Agent/TodoWrite/Task*/Web/WebFetch/NotebookEdit/Plan). Slash commands + skills (SKILL.md). Hooks. Permissions. Plan mode. Worktrees. Todos. Streaming TUI.

---

## 2. Power features

### 2.1 Routing + reasoning
Per-task/filetype/dir/subagent model pinning. Inline override (`/model X`, `@model`). Draft/editor pipeline + confidence gating. Self-consistency. `/think 0-5` mapped per provider. DeepSeek four-way (R1 hard reasoning, V4 general). Speculative parallel branches (read-only or worktree-isolated only — never mutate shared tree speculatively).

### 2.2 Looping (native primitive)
`loop while/until/accumulate/converge/poll`. Iteration = first-class object (inspect, replay, branch, kill). Budget caps (turns/tokens/$/wall-clock). Human checkpoints. Durable background loops. Nested + parallel. Early-exit + escalate.

### 2.3 Mixture of Agents (native)
Proposers (parallel blind) → aggregator → verifier. `/moa`. Auto-trigger rules — **default OFF, conservative triggers** (planner MoA-3, security MoA+verifier; do NOT MoA every call). Divergence surfaced. Agent-persona mixture. Cost caps + short-circuit on convergence. MoA inside long loops every Kth iteration.

### 2.4 Memory + knowledge
Repo embeddings + AST cache + codebase map. Hierarchical memory (working/episodic/semantic). Provenance graph ("why do you believe this?" always answerable). Failure bank. Self-improvement loop (propose rules, **dedup + conflict-detect + prune + expire** to prevent CLAUDE.md cancer). Knowledge ingestion (PDF/web/repo/browser/search).

### 2.5 Auto-CLAUDE.md / AGENTS.md
Model-tuned (markdown not XML; explicit success criteria; worked tool examples; bilingual-aware; per-repo routing hints). Hierarchy. Drift detection (propose, never silent rewrite). **Metric: real fresh-agent task success rate, not weak self-test eval.** Multi-flavor. Business profiles layer on top.

### 2.6 Swarm + A2A + multi-session
Swarm primitives native (planner→executor(s)→reviewer, worktree-isolated, parallel). Saved agent templates. Live agent monitor (kill/redirect, hard caps on count + spend). Checkpoint/resume. Agent-to-agent messaging + shared scratchpad.
- A2A: treat Hermes/Openclaw/external as PEERS. Native bidirectional async. Discovery via agent bus. Clean task-package handoff. Open protocols (MCP tools, A2A/ACP messaging) — Hermes is one peer, not load-bearing.
- Multi-session: shared blackboard (named channels), live presence, file locks, channel-scoped memory, handoff objects, follow mode, structured merge.
- Headless / Openclaw-mode: full CLI drivable from cron/launchd/scripts. Inbox/outbox over file or HTTP.

### 2.7 Tools + automation
Live diagnostics (LSP/tests/types/git auto-injected). Hooks + daemons (lint-on-save, test-runner, git-watcher). Cron. Custom tool hot-reload. Computer control (see §3.28). Time-travel filesystem (**retention policy + dedup + cap**). Custom tool builder.

### 2.8 Quality + safety
Always-on adversarial reviewer (cross-provider, default skeptic). Policy engine (declarative, model-agnostic). Confidence-aware UI. Secret-scan. Sandboxed Bash (untrusted repos). Per-model directory visibility. Audit log. WASM plugin sandbox. Causal change graph.

### 2.9 DX + UX
Session branching/replay/resume/snapshot/dry-run/diff-first. Multi-pane TUI. Voice mode. Mobile + web companions. Bilingual. Cost dashboard. Deterministic replay.

### 2.10 Infra
Provider failover + multi-key + self-host priority (opt-in, not global gate). Multi-repo + multi-tenant. OTLP observability. Live test-impact. Distributed/remote execution.

---

## 3. Maximal toolkit

### 3.1 Reasoning primitives
Tree-of-Thoughts. Self-debate. Forced reflection. Counterfactual. Socratic. Premise verification. Red-team/blue-team. Metacognitive budget. Doubt sensor. Calibration tracking. Constitution layer (editable).

### 3.2 Retrieval
Unified RAG (repo/web/email/chat/docs/issues). Hybrid BM25+dense+sparse+rerank. Multi-hop + query rewrite. Contextual compression. Lost-in-middle reordering. Watched-dir indexing. Snapshot index at git ref. Unified code graph. Semantic code search. Symbolic+neural fusion. Context budget manager.

### 3.3 Tool ecosystem
Core + infra (Docker/k8s/Terraform/AWS/GCP/Azure/Ansible) + data (postgres/mysql/mongo/redis/sqlite/clickhouse/bigquery/snowflake) + net (HTTP/gRPC/SSH/SFTP/scapy) + code (Jupyter/sandboxed REPL/regex/jq) + integrations (GitHub/GitLab/Slack/Discord/Notion/Linear/Jira/Figma/Miro/Stripe/Shopify/Vercel/Cloudflare) + comms (IMAP/SMTP/CalDAV/Meet/Zoom). Composition DSL. Versioning. Marketplace (signed). Recording/replay. Streaming. Predictive pre-fetch. Per-tool sandboxing levels. Per-tool circuit breaker.

### 3.4 Edit paradigms
Structural AST editing. Multi-cursor/multi-file coordinated. Refactor-aware (rename, extract, strategy pattern). Constraint-based (declare invariants, verify). Patches as objects. Semantic merge. Edit provenance. Repo-wide transforms. Live effect preview. Bidirectional sync (code↔spec↔docs↔tests). Generative refactors.

### 3.5 Execution + runtime
Sandboxed envs (Docker primary for v1; firecracker/gVisor/WASM later). Hot reload. Watch mode. Detached jobs. Job queue + priority + pre-emption. GPU pool (RunPod, local). Work-stealing. Reproducible envs.

### 3.6 Git (deep)
All ops native. Semantic commits. PR create/review/merge. Branch viz. Rebase helper. Conflict resolver. Commit scoring. PR descriptions auto-attached with test results. Release notes + changelog. Semver. Bisect automation. Blame-aware context. Multi-remote. Signed. Branch protection. Per-branch agent memory.

### 3.7 Testing + QA
Generation (unit/integration/e2e/property/fuzz/mutation/snapshot/visual/perf/contract). Coverage + gaps + "make this pass" mode. Flaky detection. Parallelization. Load/chaos/A-B. Quality scoring (mutation score).

### 3.8 Debugging
Time-travel. Replayable breakpoints. Variable inspection any point. Causal debugger. Stack→root-cause auto. Leak/race detection. Profiler. Log analysis. Distributed tracing. Error fingerprinting.

### 3.9 Observability
OTLP traces. Prometheus export. Dashboards. Alerting (Slack/email/PagerDuty/Telegram). SLO tracking. Cost anomaly detection. Token forecasting. Behavior replay. Audit trail (signed, exportable).

### 3.10 Security + compliance
SAST/DAST. CVE scanning. License compliance. SBOM. Secret detection. SLSA attestation. Container scan. IaC scan. Threat modeling. Pentest mode (authorized). Forensics. PII redaction. Encryption at rest. Zero-trust. Per-agent least privilege. SOC2 audit export.

### 3.11 DevOps
IaC. k8s. Compose/swarm. CI/CD authoring. Blue-green/canary/rolling. Rollback. Health checks. Runbook gen+exec. Incident assistant. Cost optimization. Capacity planning. Multi-cloud.

### 3.12 Database
Schema design. Query optimizer. Index suggestions. Safe reversible migrations. Backup/restore. Replication. Schema diff. Seed + synthetic data. PII masking. Plan viz. Slow query analysis.

### 3.13 Docs + diagrams
Auto from code. API docs (OpenAPI/GraphQL). Architecture diagrams auto. Sequence from traces. README/ADR/changelog/tutorial. Drift detection. Translation. Diagram-as-code (Mermaid/PlantUML/D2). Visual assets via image models.

### 3.14 Frontend + design
Component gen. Design system extraction. WCAG audit. Lighthouse audit. Visual/responsive testing. Bundle analysis. CSS audit. Figma↔code both ways. Storybook.

### 3.15 AI/ML engineering
Fine-tuning pipeline. Eval suite. Dataset mgmt. Training tracking. HPO. Model registry. Serving. Prompt regression. LLM judge. Embedding pipeline. Vector store mgmt. RAG builder. Agent eval.

### 3.16 Communication
Slack/Teams/Discord/Telegram bots. Email send. Calendar. Meeting summaries. Doc collab. Review assignment. Standup/status reports. Onboarding docs. KB maintenance.

### 3.17 Personalization
Learn style/preferences/risk-tolerance/verbosity. Per-user/project/business profiles. Adaptive tone/verification-depth/skill-library/memory. Style transfer.

### 3.18 Workflow + automation
Visual+code authoring. Triggers (file/git/cron/webhook/message). Marketplace. Recording→replay. Macro language. Pipeline DSL. Event-driven. MQ integration. State machines. Sagas. Compensation.

### 3.19 Performance
Speculative decoding. Prefix caching. Batch inference. Quantization. Distilled trivial-task models. Edge deployment. Streaming everything. Lazy loading. Pooling. Pre-warming.

### 3.20 QoL
Themes. Custom keybindings. Command palette. Quick actions. Context menus. Inline hints/suggestions/docs/errors. Multi-cursor. Vim/Emacs. TUI+GUI+web+mobile. Accessibility (screen reader, WCAG 2.2 AA). i18n.

### 3.21 Specialized
RE (decompile/disassemble). Binary analysis. Packet capture. Net sim. Game-dev. 3D. Audio/video/image. Notebooks. Spreadsheets. PDF. Email parsing. ICS. Geospatial. Time-series. Blockchain. IoT. Robotics. Quantum sim. Browser/Chrome/VS Code extension generation. Native app gen. Scrapers with anti-bot bypass. OCR + document AI.

### 3.22 Meta + self-modification
Agent edits own config/skills/prompts on approval. Skill auto-gen from success. Tool auto-gen from patterns. Prompt auto-optimization (bandit). Behavior+cost self-analytics. Capability discovery. Federation. Nightly self-eval → propose upgrades.

### 3.23 Sampling + inference tuning (per provider, per task)
temperature/top_p/top_k/min_p/penalties. Beam search for code. Seed control. Stop sequences. logit_bias. JSON-schema/regex/grammar-constrained output (guaranteed). Context partitioning. Per-turn token budget. Streaming chunk size. Timeout. Retry with jitter. Backoff. Circuit breaker. Bulkhead. Queue. Provider-specific defaults, user-overridable.

### 3.24 Multi-modal
Vision (screenshot, diagram→code, OCR) all providers that support. Audio (STT/TTS/music/SFX). Video (frame analysis, generation hooks). 3D (GLB inspect, mesh→code). Unified multi-modal tool.

### 3.25 Agentic commands
`/plan` `/spec` `/challenge` `/prove` `/elegant` `/pre-pr` `/fix` `/debug` `/review` `/security-review` `/test` `/refactor` `/migrate` `/optimize` `/diagram` `/doc` `/explain` `/learn` `/repl` `/replay` `/rollback` `/moa` `/redteam` `/swarm` `/loop` `/scout` `/bootstrap` `/ship`.

### 3.26 Higgsfield integration
Auto-discover MCP servers; surface every Higgsfield tool first-class (generate_image/video/audio/3d, upscale, outpaint, reframe, remove_background, motion_control, voice_change, dubbing, explainer_video, marketing_studio, virality_predictor, video_analysis, personal_clipper, shorts_studio, soul_characters, reference_elements, deploy_website, deploy_game, etc.). Hermes = A2A peer. Media-aware routing (image/video/audio → Higgsfield, not text model faking ffmpeg). Credit accounting in unified cost dashboard. Asset pipeline (spec → Higgsfield → compose → deploy). Workflow templates (brand batch, carousel, explainer, ad, photoshoot, voice clone, dubbing, clip factory). Vault-friendly logs (prompt/model/cost/seed/provenance). Browser + Higgsfield end-to-end (browse ref → extract brand → generate matching). **Lazy-load tool subsets per task to avoid schema bloat** (Part 4).

### 3.27 Obsidian integration
Vault = first-class surface (read/write/search/link/query). Bidirectional sync (session↔vault). Full semantics: wikilinks, embeds, YAML frontmatter, tags, aliases, Dataview, Canvas, Templates/Templater. Per-vault profiles (work/personal/business — compounds with §2.5). Auto-extract (session → daily note/decision/lesson/project update). Auto-ingest (vault → session grounded knowledge). Knowledge graph surfaced. Publish/preview (Quartz/Publish/MkDocs). Mobile round-trip. Per-note actions (`/note summarize|link|expand|fact-check|extract-tasks|sync-to-repo`). Creative-history bridge from Higgsfield logs.

### 3.28 Chrome control (Manus-tier + beats it)
Persistent Chrome owned via CDP + Playwright. Beats Manus via per-task routing + MoA + repo/vault/Higgsfield integration + computer-control fallback.
- Session model: save/resume/branch/share/time-travel. Persistent cookies/localStorage/IndexedDB. Multi-profile (work/personal/business). Auth handling (OAuth/2FA escalate to user, never silently fail). **Never store 2FA/TOTP secrets — escalate always** (revised from earlier, safer). Co-browse (watch + take over).
- Control: semantic-first actions, coordinate fallback. Native events (bot-detector friendly). Stealth mode (opt-in) for anti-bot. Multi-tab orchestration with tree viz. Network intercept (read/mock/replay). File ops. Extension injection. Mobile emulation + geo + proxy. Headless + headed.
- Vision+DOM fusion. Element highlighting (audit trail). Long-page handling.
- Recording/replay → macro → workflow. Cron browser jobs (directory claims, scrapes, report pulls, cross-post — absorbs Manus use cases).
- Safety: sandboxed profile/container. Blocklist never-touch domains. Destructive-action confirmation. Full audit log (URL/action/screenshot/network). Credential vault encrypted, autofill on per-session unlock only.
- Integrations: browser→code, browser→vault, browser→memory, browser→MoA, browser→swarm, browser→Hermes.

### 3.29 Creative suite
- **Framer Motion** native (animation first-class; bundle-aware).
- **UI/UX Pro Max** as default design brain (67 styles, 96 palettes, 57 font pairings, 25 charts, 13 stacks, shadcn/ui MCP).
- **Website cloning pipeline**: browser capture → defuddle/scrapling extract → design tokens reverse-engineered → reproduce as editable code (Next.js+Tailwind default) → diff/mutate mode. **IP guardrail: inspired-by default, verbatim blocked, do-not-clone blocklist, IP review gate.**
- **Remotion** (programmatic video, data-driven batches).
- **HyperFrames** (HTML video comps, short-form social).
- Creative routing matrix auto-picks (static UI / animated / clone / data-video / social-video / photoreal-ad / explainer / brand-set).
- Cross-suite compound: brand tokens flow Motion↔Remotion↔HyperFrames↔Higgsfield. Versioned in repo + mirrored to Obsidian as brand source of truth.
- Vault logging of every creative output. Browser clone-then-deploy loop (Lighthouse > 90 verify).

---

## 4. Token economy (first-class priority)

Token spend = binding constraint on autonomy. Multi-layer strategy, all layers required.

### 4.1 Caching
Prompt prefix cache (stable prefix hot). Semantic response cache (similar query → reuse, invalidated by code change). Tool result cache (hash tool+args+file mtime). Embedding cache (delta-index on git). MoA proposer cache. Repo index cache (AST + dep graph persisted). Live hit-rate UI.

### 4.2 Context minimization
Symbol-level reads (AST node, not 2000-line file). Grep-before-Read. Tool output truncation + auto-summary (full kept in cache). Sliding window + sticky facts + hierarchical summary. Context partitioning (reserve for tools/output). Lost-in-middle reordering. External store offload (facts → vault/vector, fetch on demand). Lazy skill loading (index always, body on demand). Diff-only retries. Forbid re-injecting files already in context.

### 4.3 Cost routing
Trivial → cheapest (GLM-fast/V4-quantized). General → DeepSeek V4. Agentic loops → MiniMax M3. Deep reasoning → R1. Bilingual/1M digest → GLM-5.2. Hard single-shot → GLM-5.2-think. Anthropic only on override. Auto-downshift on confidence. **Self-host = opt-in optimization, NOT global routing gate.** Off-peak routing. Distilled local for autocomplete.

### 4.4 Behavioral discipline
Plan-then-execute mandatory for M+. Verify-before-redo. Checkpoint/resume. "Do I need this?" gate before large Read/Grep. Early-stop on convergence. Budget-aware self-regulation. Forbid exploratory re-reads. Single-source-of-truth per fact.

### 4.5 Output minimization
Structured output (JSON schema) default. Terse-by-default system prompt. Max-token caps per turn/tool/response. Stop sequences. Stream + early-terminate.

### 4.6 Multi-provider batching
Batch independent calls. Speculative branches share prefix cache. MoA short-circuits on convergence. Daemons batch work.

### 4.7 Budget system
Hard caps per-turn/task/session/day/business-profile/agent. Soft warnings 50/80/95%. "Cheap mode" toggle. Cost projection before task approval. Live burn-rate. Historical analytics. Auto-pause on breach.

### 4.8 Measurement
Every turn attributed. Token-efficiency score per agent (value/spend). Weekly self-improvement pass identifies high-spend-low-value patterns. Comparative reporting (OctoCode vs stock Claude Code).

### 4.9 Provider-specific
GLM: aggressive context cache, bilingual to skip translation, prefer at scale. MiniMax M3: native MCP cuts tool-schema overhead, prefer for long loops. DeepSeek V4: cheapest strong general default; R1 only when reasoning demands, auto-downshift back. Self-hosted: free, prefer for trivial. Anthropic: expensive, override-only.

---

## 5. Anthropic policy (supported, second-class, never default)

`AnthropicProvider` against native API. Full ModelProvider — inherits features. Never default, never auto-routed. Only on explicit choice (`/model claude-*`, `@opus`, config pin). Router treats opt-in only. Cost surfaced prominently, budget caps tighter. Honest capability matrix — not special-cased as "best"; document wins AND losses (cost, no 1M context, weaker bilingual, weaker stamina than M3, weaker reasoning than R1/GLM-think on many benchmarks). No vendor flattery. Native features wired through same abstractions; gaps degrade gracefully + visibly. Telemetry isolation (only Anthropic path may phone home; others offline). No Anthropic branding in product. Provider-neutral identity.

---

## 6. Legal hygiene (non-negotiable)

**Do NOT use leaked Claude Code source — not for copying, not for "inspiration," not for reference.** "Inspired by" does not protect: copyright (substantial similarity), trade secret (misappropriation by use), ToS/CFAA exposure. Build OctoCode from scratch on the open protocol layer (MCP + A2A/ACP + OpenAI-compat) from observed behavior + public docs. Reverse-engineer the contract, not the code. The fork base (Goose, Apache-2.0) is clean — keep it clean. Preserve LICENSE + attribution; add your copyright.

---

## 7. Phasing (ship v1.0 in weeks, not months)

### v1.0 — spine (proves thesis)
Provider abstraction (GLM + MiniMax + DeepSeek + Anthropic). Router + per-task pinning + `/model`. Tool bridge semantic parity. **Token economy Part 4 (caching + minimization + budgets + dashboard).** Auto-CLAUDE.md (model-tuned, drift-detected). Plugin/SDK + marketplace foundation. TUI solid. Chrome control core. CLI-first + event bus.

### v1.1
MoA, native loops, swarm primitives. Hermes + A2A peer. Obsidian. Higgsfield (curated, lazy-loaded). Computer control (sandboxed).

### v1.2
Creative suite (Framer Motion, UI/UX Pro Max, website cloning, Remotion, HyperFrames). Multi-session coordination. Time-travel FS. Mobile/web companions.

### v2.0+
Self-improvement loop, marketplace maturity, multi-tenant/team, specialized tools long tail.

---

## 8. Scope discipline (do not violate)

- Ship v1.0 spine before kitchen sink. Cut "specialized power tools" (§3.21) to post-2.0 — those are integrations not core.
- "Beats Manus" is hubris on v1 — match core browser, beat on integration/multi-model later.
- Plugin architecture is load-bearing from day one (else you drown maintaining 200 tools). Core ships as plugins.
- CLI-first (Unix philosophy). TUI on top. GUI later. Mobile much later.
- MoA default OFF. Speculative branches read-only or worktree-isolated.
- Pick ONE sandbox for v1 (Docker). Native OS computer-control never on host by default.
- Self-improvement loop MUST dedup + conflict-detect + prune + expire (prevent CLAUDE.md cancer).
- Auto-CLAUDE.md success metric = real fresh-agent task success, not weak self-test eval.
- Tool list (Higgsfield 60+, ecosystem 200+) must lazy-load by task; never dump all schemas every turn (Part 4 token economy).
- Multi-surface (TUI+GUI+web+mobile) day one = no surface ships well. Sequence them.
- Self-host priority is opt-in per user, not global routing gate.
- Trademark "OctoCode" — verify before final commit.

---

## 9. What's already done (do not redo)

- Goose installed via Homebrew (`brew install block-goose-cli`), v1.41.0, Apache-2.0.
- Forked: github.com/drcharleskamen-png/goose → `~/Desktop/goose` (origin = fork, upstream = block/goose).
- Providers configured + verified end-to-end:
  - **GLM-5.2** (default) via Z.AI **Anthropic-compatible** endpoint (`https://api.z.ai/api/anthropic`). Reason: Z.AI's OpenAI-compat path `/api/paas/v4` 404s with goose because goose appends `/v1/chat/completions` producing `/v4/v1/...`. Anthropic route works.
  - **MiniMax-M3** via openai-compat (`OPENAI_HOST=https://api.minimaxi.chat` — no `/v1`, goose appends it).
  - **DeepSeek V4** (`deepseek-chat`) via openai-compat (`OPENAI_HOST=https://api.deepseek.com`).
  - **DeepSeek R1** (`deepseek-reasoner`) same host.
- Shell switchers in `~/.zshrc`: `goose`=GLM, `gmm`=MiniMax, `gds`=DeepSeek V4, `gdsr`=DeepSeek R1, `gcc`=Claude Code (Anthropic key not set — user priority low).
- Keys in `~/.env`: `GLM_API_KEY`, `MINIMAX_API_KEY`, `DEEPSEEK_API_KEY` (+ existing GitHub/Brave/Gemini/OpenAI/xAI/HeyGen/Manus/Webflow/ElevenLabs).
- `~/.config/goose/config.yaml` goose-generated with 14 bundled extensions enabled, `active_provider: openai`, `providers.openai.model: glm-5.2`.

---

## 10. Open questions for Charles (escalate, do not guess)

- Open-core (AGPL-style) vs proprietary SaaS licensing model for OctoCode? (Affects whether to stay on Goose Apache base or relicense strategy.)
- Trademark "OctoCode" clearance?
- Should the fork live at `drcharleskamen-png/goose` (rename later) or a new `drcharleskamen-png/octocode` repo from day one?
- First business profile target — Live Now Longevity, PepMaxx, or Octopuss? (Drives which CLAUDE.md flavor + vault profile to build first.)
