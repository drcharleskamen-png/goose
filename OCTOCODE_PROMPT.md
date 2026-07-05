# OctoCode — Build Prompt (maximal, self-contained)

You are building OctoCode — a fork of Goose (github.com/block/goose, Apache-2.0) rebuilt into the most powerful agentic coding + automation CLI ever shipped, optimized for GLM 5.2, MiniMax M3, and DeepSeek V4/R1, with Anthropic supported but deprioritized. Token economy is a first-class priority. Build on open protocols only (MCP + ACP + OpenAI-compat). Never use leaked source.

## Repo + stack
- Fork cloned at ~/Desktop/goose (origin: github.com/drcharleskamen-png/goose, upstream: block/goose).
- Goose is Rust (Cargo workspace). Code in crates/. Provider + router code is the first modification target.
- Read CLAUDE.md + AGENTS.md in the repo for Goose contributor guidance.

## Current state (DONE — do not redo)
Goose 1.41.0 installed (brew). Forked. All four providers verified end-to-end:
- GLM-5.2 (default) via Z.AI Anthropic-compat (https://api.z.ai/api/anthropic). Reason: Z.AI OpenAI-compat /api/paas/v4 404s because goose appends /v1/chat/completions producing /v4/v1/...; Anthropic route works.
- MiniMax-M3 via openai-compat (OPENAI_HOST=https://api.minimaxi.chat, no /v1).
- DeepSeek V4 (deepseek-chat) via openai-compat (OPENAI_HOST=https://api.deepseek.com).
- DeepSeek R1 (deepseek-reasoner) same host.

Shell switchers in ~/.zshrc: goose=GLM, gmm=MiniMax, gds=V4, gdsr=R1, gcc=Claude.
Keys in ~/.env: GLM_API_KEY, MINIMAX_API_KEY, DEEPSEEK_API_KEY.
Config: ~/.config/goose/config.yaml (14 bundled extensions enabled, providers.openai.model: glm-5.2).

## Hard rules (non-negotiable)
1. NO leaked Claude Code source — not for copying, not for inspiration, not for reference. Copyright (substantial similarity), trade secret (misappropriation by use), ToS/CFAA exposure all apply. Build from observed behavior + public docs only.
2. Apache-2.0 hygiene preserved. Keep LICENSE + attribution to Block; add Charles's copyright. Rebrand goose→octocode cleanly when ready.
3. Token economy is priority from v1.0 (see Part 4). Every feature justifies its token cost.
4. Ship v1.0 spine before maximal toolkit. Cut §3.21 specialized tools to post-2.0.
5. Plugin architecture load-bearing from day one. Core ships as plugins.
6. CLI-first (Unix). TUI on top. GUI later. Mobile much later.
7. Security defaults conservative. Sandbox untrusted tools/computer-control. Secret-scan. Destructive actions confirm. Never store 2FA secrets. WASM sandbox plugins.
8. Escalate open questions — don't guess (see Open Questions).

---

## PART 1 — ARCHITECTURE

A. Provider abstraction — ModelProvider interface (stream/complete/count_tokens/supports_tools/supports_reasoning/max_context/cache_strategy/cost_per_token). GLM/MiniMax/DeepSeek native first; Anthropic native, supported, never default.
B. Tool-calling bridge — provider-native formats (GLM/DeepSeek function-calling; MiniMax native tool-use + MCP; Anthropic tool_use). Identical tool SEMANTICS across providers. Aggressive parallel tool calls.
C. Per-provider system prompt builder — canonical instructions wrapped model-natively (GLM: structure + worked examples + bilingual; M3: agentic framing; V4: standard; R1: reasoning cues + show-work).
D. Context window + smart compaction — ~1M default, auto-compact ~700k, keep full contents in-context, diff-aware retry, smart compaction preserves tool I/O.
E. Prompt caching — provider-specific (GLM/MiniMax/DeepSeek/Anthropic). Stable prefix cached. Cache-hit UI + manual warming.
F. Tokenizer + accounting — provider tokenizers. Accurate compaction + cost.
G. Reasoning/thinking — GLM thinking, M3 hybrid budget, R1 reasoning, Anthropic ext. /think 0-5. Auto-escalate on retry.
H. Subagents + parallelism — high concurrency defaults. Planner→executor→reviewer. Worktree isolation. run_in_background.
I. MCP — all providers; MiniMax consumes natively.
J. Harness preservation — full tool set (Read/Edit/Bash/Grep/Glob/Agent/TodoWrite/Task*/Web/WebFetch/NotebookEdit/Plan), slash commands, skills (SKILL.md), hooks, permissions, plan mode, worktrees, todos, streaming TUI.

---

## PART 2 — POWER FEATURES

2.1 Routing + reasoning — per-task/filetype/dir/subagent model pinning; inline override (/model X, @model); draft/editor + confidence gating; self-consistency; /think 0-5; DeepSeek four-way (R1 hard, V4 general); speculative parallel branches (read-only or worktree-isolated ONLY — never mutate shared tree speculatively).

2.2 Looping (native) — loop while/until/accumulate/converge/poll; iteration = first-class object; budget caps; human checkpoints; durable background loops; nested + parallel; early-exit + escalate.

2.3 Mixture of Agents (native) — proposers (parallel blind) → aggregator → verifier; /moa; auto-trigger rules DEFAULT OFF, conservative (planner MoA-3, security MoA+verifier; do NOT MoA every call); divergence surfaced; agent-persona mixture; cost caps + short-circuit on convergence; MoA in long loops every Kth iteration.

2.4 Memory + knowledge — repo embeddings + AST cache + codebase map; hierarchical memory (working/episodic/semantic); provenance graph ("why do you believe this?" always answerable); failure bank; self-improvement loop (propose rules with DEDUP + CONFLICT-DETECT + PRUNE + EXPIRE to prevent CLAUDE.md cancer); knowledge ingestion (PDF/web/repo/browser/search).

2.5 Auto-CLAUDE.md/AGENTS.md — model-tuned (markdown not XML; explicit success criteria; worked tool examples; bilingual-aware; per-repo routing hints); hierarchy; drift detection (propose, never silent rewrite); METRIC = real fresh-agent task success rate (not weak self-test eval); multi-flavor; business profiles layer on top.

2.6 Swarm + A2A + multi-session — swarm primitives native (planner→executor(s)→reviewer, worktree-isolated, parallel); saved agent templates; live agent monitor (kill/redirect, hard caps on count + spend); checkpoint/resume; agent-to-agent messaging + shared scratchpad. A2A: treat Hermes/Openclaw/external as PEERS; native bidirectional async; discovery via agent bus; clean task-package handoff; open protocols (MCP tools, A2A/ACP messaging) — Hermes is one peer, not load-bearing. Multi-session: shared blackboard (named channels), live presence, file locks, channel-scoped memory, handoff objects, follow mode, structured merge. Headless/Openclaw-mode: full CLI drivable from cron/launchd/scripts; inbox/outbox over file or HTTP.

2.7 Tools + automation — live diagnostics (LSP/tests/types/git auto-injected); hooks + daemons (lint-on-save, test-runner, git-watcher); cron; custom tool hot-reload; computer control (see 3.28); time-travel filesystem (retention policy + dedup + cap); custom tool builder.

2.8 Quality + safety — always-on adversarial reviewer (cross-provider, default skeptic); policy engine (declarative, model-agnostic); confidence-aware UI; secret-scan; sandboxed Bash (untrusted repos); per-model directory visibility; audit log; WASM plugin sandbox; causal change graph.

2.9 DX + UX — session branching/replay/resume/snapshot/dry-run/diff-first; multi-pane TUI; voice mode; mobile + web companions; bilingual; cost dashboard; deterministic replay.

2.10 Infra — provider failover + multi-key + self-host priority (opt-in, NOT global gate); multi-repo + multi-tenant; OTLP observability; live test-impact; distributed/remote execution.

---

## PART 3 — MAXIMAL TOOLKIT

3.1 Reasoning primitives — Tree-of-Thoughts; self-debate; forced reflection; counterfactual; Socratic; premise verification; red-team/blue-team; metacognitive budget; doubt sensor; calibration tracking; constitution layer (editable).

3.2 Retrieval — unified RAG (repo/web/email/chat/docs/issues); hybrid BM25+dense+sparse+rerank; multi-hop + query rewrite; contextual compression; lost-in-middle reordering; watched-dir indexing; snapshot index at git ref; unified code graph; semantic code search; symbolic+neural fusion; context budget manager.

3.3 Tool ecosystem — core + infra (Docker/k8s/Terraform/AWS/GCP/Azure/Ansible) + data (postgres/mysql/mongo/redis/sqlite/clickhouse/bigquery/snowflake) + net (HTTP/gRPC/SSH/SFTP/scapy) + code (Jupyter/sandboxed REPL/regex/jq) + integrations (GitHub/GitLab/Slack/Discord/Notion/Linear/Jira/Figma/Miro/Stripe/Shopify/Vercel/Cloudflare) + comms (IMAP/SMTP/CalDAV/Meet/Zoom). Composition DSL. Versioning. Marketplace (signed). Recording/replay. Streaming. Predictive pre-fetch. Per-tool sandboxing levels. Per-tool circuit breaker. LAZY-LOAD per task (token economy).

3.4 Edit paradigms — structural AST editing; multi-cursor/multi-file coordinated; refactor-aware (rename, extract, strategy pattern); constraint-based (declare invariants, verify); patches as objects; semantic merge; edit provenance; repo-wide transforms; live effect preview; bidirectional sync (code↔spec↔docs↔tests); generative refactors.

3.5 Execution + runtime — sandboxed envs (Docker PRIMARY for v1; firecracker/gVisor/WASM later); hot reload; watch mode; detached jobs; job queue + priority + pre-emption; GPU pool (RunPod, local); work-stealing; reproducible envs.

3.6 Git (deep) — all ops native; semantic commits; PR create/review/merge; branch viz; rebase helper; conflict resolver; commit scoring; PR descriptions with test results; release notes + changelog; semver; bisect automation; blame-aware context; multi-remote; signed; branch protection; per-branch agent memory.

3.7 Testing + QA — generation (unit/integration/e2e/property/fuzz/mutation/snapshot/visual/perf/contract); coverage + gaps + "make this pass" mode; flaky detection; parallelization; load/chaos/A-B; quality scoring (mutation score).

3.8 Debugging — time-travel; replayable breakpoints; variable inspection any point; causal debugger; stack→root-cause auto; leak/race detection; profiler; log analysis; distributed tracing; error fingerprinting.

3.9 Observability — OTLP traces; Prometheus export; dashboards; alerting (Slack/email/PagerDuty/Telegram); SLO tracking; cost anomaly detection; token forecasting; behavior replay; audit trail (signed, exportable).

3.10 Security + compliance — SAST/DAST; CVE scanning; license compliance; SBOM; secret detection; SLSA attestation; container scan; IaC scan; threat modeling; pentest mode (authorized); forensics; PII redaction; encryption at rest; zero-trust; per-agent least privilege; SOC2 audit export.

3.11 DevOps — IaC; k8s; compose/swarm; CI/CD authoring; blue-green/canary/rolling; rollback; health checks; runbook gen+exec; incident assistant; cost optimization; capacity planning; multi-cloud.

3.12 Database — schema design; query optimizer; index suggestions; safe reversible migrations; backup/restore; replication; schema diff; seed + synthetic data; PII masking; plan viz; slow query analysis.

3.13 Docs + diagrams — auto from code; API docs (OpenAPI/GraphQL); architecture diagrams auto; sequence from traces; README/ADR/changelog/tutorial; drift detection; translation; diagram-as-code (Mermaid/PlantUML/D2); visual assets via image models.

3.14 Frontend + design — component gen; design system extraction; WCAG audit; Lighthouse audit; visual/responsive testing; bundle analysis; CSS audit; Figma↔code both ways; Storybook.

3.15 AI/ML engineering — fine-tuning pipeline; eval suite; dataset mgmt; training tracking; HPO; model registry; serving; prompt regression; LLM judge; embedding pipeline; vector store mgmt; RAG builder; agent eval.

3.16 Communication — Slack/Teams/Discord/Telegram bots; email send; calendar; meeting summaries; doc collab; review assignment; standup/status reports; onboarding docs; KB maintenance.

3.17 Personalization — learn style/preferences/risk-tolerance/verbosity; per-user/project/business profiles; adaptive tone/verification-depth/skill-library/memory; style transfer.

3.18 Workflow + automation — visual+code authoring; triggers (file/git/cron/webhook/message); marketplace; recording→replay; macro language; pipeline DSL; event-driven; MQ integration; state machines; sagas; compensation.

3.19 Performance — speculative decoding; prefix caching; batch inference; quantization; distilled trivial-task models; edge deployment; streaming everything; lazy loading; pooling; pre-warming.

3.20 QoL — themes; custom keybindings; command palette; quick actions; context menus; inline hints/suggestions/docs/errors; multi-cursor; Vim/Emacs; TUI+GUI+web+mobile; accessibility (screen reader, WCAG 2.2 AA); i18n.

3.21 Specialized (post-2.0) — RE/binary analysis; packet capture; net sim; game-dev; 3D; audio/video/image; notebooks; spreadsheets; PDF; email parsing; ICS; geospatial; time-series; blockchain; IoT; robotics; quantum sim; extension gen; native app gen; scrapers with anti-bot bypass; OCR + document AI.

3.22 Meta + self-modification — agent edits own config/skills/prompts on approval; skill auto-gen from success; tool auto-gen from patterns; prompt auto-optimization (bandit); behavior+cost self-analytics; capability discovery; federation; nightly self-eval → propose upgrades.

3.23 Sampling + inference tuning (per provider, per task) — temperature/top_p/top_k/min_p/penalties; beam search for code; seed control; stop sequences; logit_bias; JSON-schema/regex/grammar-constrained output (guaranteed); context partitioning; per-turn token budget; streaming chunk size; timeout; retry with jitter; backoff; circuit breaker; bulkhead; queue; provider-specific defaults, user-overridable.

3.24 Multi-modal — vision (screenshot, diagram→code, OCR) all providers that support; audio (STT/TTS/music/SFX); video (frame analysis, generation hooks); 3D (GLB inspect, mesh→code); unified multi-modal tool.

3.25 Agentic commands — /plan /spec /challenge /prove /elegant /pre-pr /fix /debug /review /security-review /test /refactor /migrate /optimize /diagram /doc /explain /learn /repl /replay /rollback /moa /redteam /swarm /loop /scout /bootstrap /ship.

3.26 Higgsfield integration — auto-discover MCP servers; surface every Higgsfield tool first-class (generate_image/video/audio/3d, upscale, outpaint, reframe, remove_background, motion_control, voice_change, dubbing, explainer_video, marketing_studio, virality_predictor, video_analysis, personal_clipper, shorts_studio, soul_characters, reference_elements, deploy_website, deploy_game, etc.); Hermes = A2A peer; media-aware routing (image/video/audio → Higgsfield, not text model faking ffmpeg); credit accounting in unified cost dashboard; asset pipeline (spec → Higgsfield → compose → deploy); workflow templates (brand batch, carousel, explainer, ad, photoshoot, voice clone, dubbing, clip factory); vault-friendly logs (prompt/model/cost/seed/provenance); browser + Higgsfield end-to-end (browse ref → extract brand → generate matching). LAZY-LOAD tool subsets per task.

3.27 Obsidian integration — vault = first-class surface (read/write/search/link/query); bidirectional sync (session↔vault); full semantics (wikilinks, embeds, YAML frontmatter, tags, aliases, Dataview, Canvas, Templates/Templater); per-vault profiles (work/personal/business); auto-extract (session → daily note/decision/lesson/project update); auto-ingest (vault → session grounded knowledge); knowledge graph surfaced; publish/preview (Quartz/Publish/MkDocs); mobile round-trip; per-note actions (/note summarize|link|expand|fact-check|extract-tasks|sync-to-repo); creative-history bridge from Higgsfield logs.

3.28 Chrome control (Manus-tier + beats it) — persistent Chrome owned via CDP + Playwright; beats Manus via per-task routing + MoA + repo/vault/Higgsfield integration + computer-control fallback. Session model: save/resume/branch/share/time-travel; persistent cookies/localStorage/IndexedDB; multi-profile (work/personal/business); auth handling (OAuth/2FA ESCALATE TO USER, never silently fail); NEVER store 2FA/TOTP secrets — escalate always; co-browse (watch + take over). Control: semantic-first actions, coordinate fallback; native events (bot-detector friendly); stealth mode (opt-in); multi-tab orchestration with tree viz; network intercept (read/mock/replay); file ops; extension injection; mobile emulation + geo + proxy; headless + headed. Vision+DOM fusion; element highlighting; long-page handling. Recording/replay → macro → workflow; cron browser jobs (directory claims, scrapes, report pulls, cross-post — absorbs Manus use cases). Safety: sandboxed profile/container; blocklist never-touch domains; destructive-action confirmation; full audit log (URL/action/screenshot/network); credential vault encrypted, autofill on per-session unlock only. Integrations: browser→code, browser→vault, browser→memory, browser→MoA, browser→swarm, browser→Hermes.

3.29 Creative suite — Framer Motion native (animation first-class, bundle-aware); UI/UX Pro Max as default design brain (67 styles, 96 palettes, 57 font pairings, 25 charts, 13 stacks, shadcn/ui MCP); website cloning pipeline (browser capture → defuddle/scrapling extract → design tokens reverse-engineered → reproduce as editable code Next.js+Tailwind default → diff/mutate mode; IP GUARDRAIL: inspired-by default, verbatim blocked, do-not-clone blocklist, IP review gate); Remotion (programmatic video, data-driven batches); HyperFrames (HTML video comps, short-form social); creative routing matrix auto-picks (static UI / animated / clone / data-video / social-video / photoreal-ad / explainer / brand-set); cross-suite compound (brand tokens flow Motion↔Remotion↔HyperFrames↔Higgsfield, versioned in repo + mirrored to Obsidian as brand source of truth); vault logging of every creative output; browser clone-then-deploy loop (Lighthouse > 90 verify).

---

## PART 4 — TOKEN ECONOMY (first-class priority)

Token spend = binding constraint on autonomy. All layers required.

4.1 Caching — prompt prefix cache (stable prefix hot); semantic response cache (similar query → reuse, invalidated by code change); tool result cache (hash tool+args+file mtime); embedding cache (delta-index on git); MoA proposer cache; repo index cache (AST + dep graph persisted); live hit-rate UI.

4.2 Context minimization — symbol-level reads (AST node, not 2000-line file); grep-before-read; tool output truncation + auto-summary (full kept in cache); sliding window + sticky facts + hierarchical summary; context partitioning (reserve for tools/output); lost-in-middle reordering; external store offload (facts → vault/vector, fetch on demand); lazy skill loading (index always, body on demand); diff-only retries; forbid re-injecting files already in context.

4.3 Cost routing — trivial → cheapest (GLM-fast/V4-quantized); general → DeepSeek V4; agentic loops → MiniMax M3; deep reasoning → R1; bilingual/1M digest → GLM-5.2; hard single-shot → GLM-5.2-think; Anthropic only on override; auto-downshift on confidence; SELF-HOST = OPT-IN optimization, NOT global routing gate; off-peak routing; distilled local for autocomplete.

4.4 Behavioral discipline — plan-then-execute mandatory for M+; verify-before-redo; checkpoint/resume; "do I need this?" gate before large read/grep; early-stop on convergence; budget-aware self-regulation; forbid exploratory re-reads; single-source-of-truth per fact.

4.5 Output minimization — structured output (JSON schema) default; terse-by-default system prompt; max-token caps per turn/tool/response; stop sequences; stream + early-terminate.

4.6 Multi-provider batching — batch independent calls; speculative branches share prefix cache; MoA short-circuits on convergence; daemons batch work.

4.7 Budget system — hard caps per-turn/task/session/day/business-profile/agent; soft warnings 50/80/95%; "cheap mode" toggle; cost projection before task approval; live burn-rate; historical analytics; auto-pause on breach.

4.8 Measurement — every turn attributed (tokens in/out, cache hit/miss, model, cost, tool calls); token-efficiency score per agent (value/spend); weekly self-improvement pass identifies high-spend-low-value patterns; comparative reporting (OctoCode vs stock Claude Code). Target: ≥60% token reduction at equal/better quality.

4.9 Provider-specific — GLM: aggressive context cache, bilingual to skip translation, prefer at scale. MiniMax M3: native MCP cuts tool-schema overhead, prefer for long loops. DeepSeek V4: cheapest strong general default; R1 only when reasoning demands, auto-downshift back. Self-hosted: free, prefer for trivial. Anthropic: expensive, override-only.

---

## ANTHROPIC POLICY (supported, second-class, never default)

AnthropicProvider against native API. Full ModelProvider — inherits features. Never default, never auto-routed. Only on explicit choice (/model claude-*, @opus, config pin). Router treats opt-in only. Cost surfaced prominently, budget caps tighter. Honest capability matrix — NOT special-cased as "best"; document wins AND losses (cost, no 1M context, weaker bilingual, weaker stamina than M3, weaker reasoning than R1/GLM-think on many benchmarks). No vendor flattery. Native features wired through same abstractions; gaps degrade gracefully + visibly. Telemetry isolation (only Anthropic path may phone home; others offline-capable against self-hosted). No Anthropic branding in product. Provider-neutral identity.

---

## LEGAL HYGIENE (non-negotiable)

Do NOT use leaked Claude Code source — not for copying, not for "inspiration," not for reference. "Inspired by" does not protect: copyright (substantial similarity), trade secret (misappropriation by use), ToS/CFAA exposure. Build OctoCode from scratch on the open protocol layer (MCP + A2A/ACP + OpenAI-compat) from observed behavior + public docs. Reverse-engineer the contract, not the code. The fork base (Goose, Apache-2.0) is clean — keep it clean. Preserve LICENSE + attribution; add Charles's copyright.

---

## PHASING (ship v1.0 in weeks, not months)

v1.0 — spine: provider abstraction (GLM+MiniMax+DeepSeek+Anthropic); router + per-task pinning + /model; tool bridge semantic parity; TOKEN ECONOMY Part 4 (caching + minimization + budgets + dashboard); auto-CLAUDE.md (model-tuned, drift-detected); plugin/SDK + marketplace foundation; TUI solid; Chrome control core; CLI-first + event bus.

v1.1 — MoA; native loops; swarm primitives; Hermes + A2A peer; Obsidian; Higgsfield (curated, lazy-loaded); computer control (sandboxed).

v1.2 — creative suite (Framer Motion, UI/UX Pro Max, website cloning, Remotion, HyperFrames); multi-session coordination; time-travel FS; mobile/web companions.

v2.0+ — self-improvement loop; marketplace maturity; multi-tenant/team; specialized tools long tail.

---

## SCOPE DISCIPLINE (do not violate)

- Ship v1.0 spine before kitchen sink. Cut §3.21 to post-2.0.
- "Beats Manus" deferred — match core browser first, beat on integration later.
- Plugin architecture load-bearing from day one. Core ships as plugins.
- CLI-first. TUI on top. GUI later. Mobile much later.
- MoA default OFF. Speculative branches read-only or worktree-isolated.
- ONE sandbox for v1 (Docker). Native OS computer-control never on host by default.
- Self-improvement loop MUST dedup + conflict-detect + prune + expire.
- Auto-CLAUDE.md metric = real fresh-agent task success, not weak self-test eval.
- Tool list (Higgsfield 60+, ecosystem 200+) MUST lazy-load by task.
- Multi-surface sequenced, not simultaneous.
- Self-host priority = opt-in per user, not global routing gate.
- Trademark "OctoCode" — verify before final commit.

---

## OPEN QUESTIONS (escalate, do not guess)

1. Open-core (AGPL-style, community + paid hosted) vs proprietary SaaS licensing?
2. Trademark "OctoCode" clearance?
3. Repo: keep drcharleskamen-png/goose and rename later, or new octocode repo from day one?
4. First business profile target — Live Now Longevity, PepMaxx, or Octopuss? (Drives which CLAUDE.md flavor + vault profile to build first.)

---

## FIRST TASK (concrete, before anything else)

1. cd ~/Desktop/goose && cargo build — verify toolchain works.
2. Read CLAUDE.md + AGENTS.md. Map crates/ structure. Find provider + router code (crates/goose/src/providers/ likely).
3. Produce a 1-page architecture map: where provider abstraction lives, where routing lives, where token accounting happens (if any yet), where hooks/extensions plug in, where system prompt is constructed, where tool-call transport is. This map drives every modification.
4. Get Charles's sign-off on the map.
5. Implement v1.0 spine in this order: provider abstraction generalization → router with per-task pinning → tool bridge parity → TOKEN ECONOMY LAYER (Part 4) → auto-CLAUDE.md → plugin/SDK foundation → Chrome control core.

---

## COMMUNICATION

Status: commit regularly + maintain OCTOCODE_LOG.md (you create) with what changed, why, token-cost impact, next step. Escalate blockers + open questions immediately — do not improvise past architectural decisions.

Charles cares about most: token efficiency (≥60% reduction vs stock Claude Code), real-task quality on his businesses (Live Now Longevity peptide clinic, PepMaxx Labs, Octopuss AI), autonomous operation (he runs Openclaw swarm), compounding memory (Obsidian + auto-CLAUDE.md + self-improvement). Build for those, not for benchmarks.

Begin.
