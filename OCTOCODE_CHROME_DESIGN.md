# OctoCode Chrome Control — Design Proposal (spine item 7)

Status: **proposal + recon, awaiting Charles sign-off.** Not implemented.
Read-only recon of the existing surface; implementation needs the open
questions answered and (critically) working build disk.

## 0. TL;DR

Goose's existing "computer control" is **macOS-native, coordinate-based**:
AppleScript via `system_automation.execute_system_script` + the external
`peekaboo` brew package (screen capture / OCR / element finding). There is
**no browser automation** — no CDP, no Playwright, no chromium, no
fantoccini/thirtyfour/chromiumoxide in any `Cargo.toml`. Spec §3.28
(Manus-tier Chrome) is therefore **greenfield**, not extension.

Per spec §8 scope discipline: v1 = **match core browser** (reliable CDP-driven
navigation, multi-tab, persistent sessions, auth). "Beats Manus" via MoA /
routing / vault / Higgsfield integration is **deferred to v1.1+**.

## 1. What goose already has (grounded)

`crates/goose-mcp/src/computercontroller/mod.rs` exposes (macOS-primary):
- `computer_control` / `automation_script` — run platform "system scripts"
  (AppleScript on mac) through `system_automation.execute_system_script`.
- `web_scrape` — HTTP fetch + parse of a page (no rendering, no JS).
- `save_to_cache` / `cache` / `list_resources` / `read_resource` — output cache
  surfaced back to the model as MCP resources.
- `xlsx_tool` / `docx_tool` / `pdf_tool` — doc format handlers (unrelated to
  browsing).
- `peekaboo_impl` + `crates/goose-mcp/src/peekaboo/` — brew package
  auto-installer; provides screen capture + OCR + element finding **for the
  native macOS desktop**, not for a browser DOM.

So today the model can: scrape a static URL, run an AppleScript, and (on mac)
OCR the screen. It **cannot** drive a real browser session, persist login,
orchestrate tabs, or intercept network. That gap is spine item 7.

## 2. Gap analysis vs spec §3.28

| §3.28 requirement | Status |
|---|---|
| Persistent Chrome via CDP + Playwright | **missing** |
| Session model: save / resume / branch / share / time-travel | **missing** |
| Persistent cookies / localStorage / IndexedDB | **missing** |
| Multi-profile (work / personal / business) | **missing** |
| Auth handling (OAuth / 2FA escalate, never store 2FA) | **missing** |
| Multi-tab orchestration with tree viz | **missing** |
| Network intercept (read / mock / replay) | **missing** |
| Semantic-first actions + coordinate fallback | partial (peekaboo coordinates only) |
| Native events (bot-detector friendly) | **missing** |
| Stealth mode (opt-in, anti-bot) | **missing** |
| Recording / replay → macro → workflow | **missing** |
| Cron browser jobs | skeleton only (goose scheduler exists, no browser job) |
| Sandboxed profile + blocklist + audit log + credential vault | **missing** |
| Browser↔code / vault / memory / MoA / swarm / Higgsfield | **missing** |

## 3. Proposed architecture

**One new crate, `crates/goose-mcp/src/browser/`** exposing an MCP extension
that internally drives a real browser.

### 3.1 Driver choice (escalate)

| Option | Pros | Cons |
|---|---|---|
| **`chromiumoxide`** (Rust CDP client) | Pure Rust, tokio-native, mature, no Node dep | Lower-level; we build session/profile mgmt ourselves |
| **`fantoccini`** (Rust WebDriver client) | Stable, protocol-standard | Needs a WebDriver binary; less CDP coverage |
| **`headless_chrome`** (Rust CDP) | Simple | Less active; weaker for stealth |
| **Playwright via `playwright` crate** (Node under hood) | Best stealth, network mock, multi-browser | Pulls Node runtime; heavier; cross-process |

**Recommendation:** `chromiumoxide` for v1 — pure-Rust, no Node dependency
(keeps the CLI single-binary), tokio-native (matches goose's runtime), full
CDP for tabs/network/DOM. Revisit Playwright if stealth/anti-bot becomes the
dominant use case. Escalation point in §7.

### 3.2 Session model

A `BrowserSession` is a directory under `~/.config/goose/browser/sessions/<id>/`:
- `User Data` dir (Chromium profile: cookies, localStorage, IndexedDB, cache).
- `session.yaml` — metadata: profile kind (work/personal/business), created,
  last-used, blocklist snapshot, consent flags.
- `audit.log` — append-only: URL / action / screenshot path / network summary
  per step (spec §3.28 safety).

Sessions support: `save` (snapshot current state), `resume` (reopen the
profile dir), `branch` (copy-on-write clone for speculative flows),
`share` (export tarball, secrets redacted), `time-travel` (rotate profile
snapshots on a retention policy — mirrors spec time-travel FS guardrails).

### 3.3 Credential vault

Encrypted store (OS keyring via the existing `keyring` pattern goose uses for
provider keys) keyed by `(profile, domain)`. Autofill on session unlock only.
**Never store 2FA/TOTP secrets** (spec §3.28 revision) — when a 2FA field is
detected, pause + escalate to the user; never silently fail.

### 3.4 Safety (spec §3.28 + §2.8)

- Sandboxed Chromium profile per session (no shared default profile).
- `never-touch` domain blocklist in config; loader refuses navigations there.
- Destructive actions (form submit on a banking/payments domain, delete,
  bulk send) require explicit confirmation — same confirm primitive goose
  uses for destructive shell.
- Audit log signed + exportable.
- Untrusted-site mode: run the Chromium instance inside the v1 Docker sandbox
  (spec §8 single-sandbox rule); no host FS.

### 3.5 Lazy tool loading (Part 4 integration)

Browser tools (navigate, click, type, screenshot, intercept, etc.) are many
and schema-heavy. Per plugin-design §6, they ship `lazy: true` — only
activated when the router detects a browser task. Off-browser turns never see
browser schemas. This is the mechanism that keeps browser control affordable
when the model is just editing code.

## 4. Tool surface (v1 — match core browser)

Minimum viable, all CDP-driven:
- `browser_open(url)` / `browser_navigate(url)` — open in a (new) tab.
- `browser_click(selector)` — semantic-first; coordinate fallback.
- `browser_type(selector, text)` — incl. special keys.
- `browser_screenshot(...)` — full page or element; vision-model friendly.
- `browser_read(dom|text|aria)` — extract structured page content (replaces
  ad-hoc `web_scrape` for rendered pages).
- `browser_tab_list` / `browser_tab_focus` / `browser_tab_close`.
- `browser_session_save|resume|branch|list`.
- `browser_wait_for(selector|nav|ms)`.

**Deferred to v1.1+** (the "beats Manus" surface): network intercept/mock/
replay, stealth mode, mobile emulation + geo + proxy, extension injection,
recording→macro→workflow, co-browse, browser→MoA→swarm wiring.

## 5. Integration points

- **Router (spine 2):** browser tasks route to a vision-capable model
  (GLM-5.2, MiniMax-M3) — DeepSeek V4 text-only is a poor fit for screenshot
  reasoning. Per-task pinning already supports this.
- **Plugin system (spine 6):** ship browser as a first-party plugin
  (`lazy: true`), proving the plugin contract on a real complex extension.
- **Higgsfield (spec §3.26):** browser-capture → brand-extract → Higgsfield
  generate pipeline (website cloning, §3.29). v1.1+.
- **Vault (spec §3.27):** session↔vault sync (daily note, decision, lesson).
  v1.1+.

## 6. Phasing inside v1.0

1. **Crate scaffold + `chromiumoxide` driver + `browser_open/navigate/screenshot/read`.**
   Smallest end-to-end loop (model can load + read a rendered page).
2. **Session model** (§3.2) — save/resume/branch + persistent cookies.
3. **Click/type/tab tools** — full core-browser interaction.
4. **Credential vault + blocklist + audit log** (§3.4) — safety gate before
   any real authed use.
5. **Lazy tool loading wiring** (plugin §6) — token economy.

Each phase independently useful. Phases 1–3 = "match core browser"; phase 4 =
safe enough for daily driver; phase 5 = Part-4 discipline.

## 7. Open questions for Charles (escalate before implementing)

1. **Driver:** confirm `chromiumoxide` (pure-Rust, single-binary CLI) vs
   Playwright-rust (best stealth, pulls Node)? I recommend chromiumoxide.
2. **Platform scope for v1:** macOS-only first (matches peekaboo), or
   macOS+Linux from day one? Windows later regardless.
3. **Relationship to existing `peekaboo` + AppleScript `computer_control`:**
   keep both (native-desktop control is a different use case from browser
   control), deprecate, or fold peekaboo under the new browser crate as the
   "native desktop" fallback? I recommend **keep both** — browser is additive.
4. **Stealth/anti-bot in v1 or v1.1?** Spec marks it opt-in; recommend v1.1
   (keep v1 clean, sites that need stealth are a distinct workflow).
5. **Persistent-profile storage location + encryption-at-rest:** default
   `~/.config/goose/browser/sessions/` plaintext vs encrypted-blob dir?
   Sensitive (cookies = session tokens). Recommend encrypted-at-rest from
   phase 4.
6. **Trademark:** "browser" tool names are generic; no OctoCode-trademark
   dependency here. But the crate name `goose-mcp/browser` assumes the rename
   timeline — confirm we keep `goose-*` paths until trademark clears (§10).

## 8. What this proposal does NOT do (scope discipline)

- No "beats Manus" in v1 (spec §8: hubris). Match core browser first.
- No network intercept/mock/replay in v1.
- No stealth/anti-bot, mobile emulation, proxy rotation, extension injection
  in v1.
- No browser→macro→workflow recorder, no cron browser jobs, no co-browse in v1.
- No replacement of the existing peekaboo/AppleScript native-desktop control.

## 9. Hard dependency

Implementation requires **working build disk**. The `chromiumoxide` dep + its
transitive `tokio-tungstenite` / `proc-macro` chain adds non-trivial compile
weight; at the current 2–6 GB free, the build will ENOSPC mid-compile. Spine
item 7 is blocked on the same permanent disk fix (external `CARGO_TARGET_DIR`
or grown APFS container) as the release rebuild.
