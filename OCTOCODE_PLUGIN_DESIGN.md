# OctoCode Plugin/SDK Foundation — Design Proposal (spine item 6)

Status: **proposal, awaiting Charles sign-off.** Not implemented. Mirrors the
archmap → sign-off → implement pattern. Per handoff rule, do not improvise past
architectural decisions; the open questions at the end need answers first.

## 0. TL;DR

Goose already ships a capable, MCP-based plugin system under the name
"extensions." OctoCode v1.0's job is to **formalize, document, and add a
marketplace + SDK + signed-install flow** on top of it — not to invent a new
plugin runtime. WASM sandbox is deferred to post-v1 (spec §8: one sandbox for
v1 = Docker; untrusted plugins get process isolation, which Stdio MCP already
provides).

## 1. What goose already has (grounded, read from source)

`crates/goose/src/agents/extension.rs` defines `ExtensionConfig`, a tagged enum:

| Variant | Transport | Trust | Where it runs |
|---|---|---|---|
| `Builtin` | in-process | trusted | bundled goose-mcp tools (developer, etc.) |
| `Platform` | in-process | trusted | direct agent access (code_execution, summon, …) — `PLATFORM_EXTENSIONS` map |
| `Stdio` | MCP over stdin/stdout | **process-isolated** | any external binary |
| `StreamableHttp` | MCP over HTTP(S) / UDS | network-isolated | remote MCP server |
| `InlinePython` | `uvx` subprocess | **process-isolated** | ad-hoc Python snippet |
| `Frontend` | UI RPC | trusted | tools supplied by the desktop UI |
| `Sse` | — | unsupported (kept for compat) | — |

Security primitives already present:
- `Envs` blocks 31 dangerous env vars (`LD_PRELOAD`, `DYLD_INSERT_LIBRARIES`,
  `PATH`, `PYTHONPATH`, `NODE_OPTIONS`, …) — `extension.rs:81`.
- `available_tools: Vec<String>` per extension — empty = all tools, else
  allowlist. **This is already the token-economy lever for lazy tool loading**
  (spec §3.26/§8: "tool list must lazy-load by task").
- `permission: PermissionLevel` on `ToolInfo`.
- Config-driven: extensions live in `~/.config/goose/config.yaml`, resolved with
  env-var substitution (`$KEY`) for secrets.

So: transport, isolation, permissions, env hygiene, per-tool filtering, and a
config registry **all exist**. The plugin contract is implicit in
`ExtensionConfig`; we make it explicit.

## 2. Gap analysis (what v1.0 must add)

1. **No plugin manifest / metadata.** An extension is just a config stanza;
   there's no version, author, license, source URL, signature, or declared
   permission scope.
2. **No install flow.** Adding an extension = hand-editing config.yaml. No
   `install`, `list`, `remove`, `upgrade`, no dependency resolution.
3. **No signing / verification.** A Stdio extension runs any binary from any
   path. Marketplace distribution needs signed packages + checksum verify.
4. **No SDK.** Authors write raw MCP servers from scratch; no opinionated
   helper for the common OctoCode plugin shape (declare tools + permissions +
   system-prompt snippet, get a working plugin).
5. **No lazy tool loading by task.** `available_tools` is static per extension;
   spec §4.2/§8 wants the *active* tool set scoped per turn to cut schema
   bloat (the Higgsfield-60+/ecosystem-200+ problem).

## 3. Proposed contract: `octocode-plugin.yaml`

A plugin = a directory with a manifest + (optional) code/assets. Manifest is a
strict superset of one `ExtensionConfig` stanza + metadata:

```yaml
# octocode-plugin.yaml
api_version: 1
name: livenow-booking
version: 0.1.0
author: Charles Kamen <charles@livenowlongevity.com>
license: Apache-2.0
description: Bookings + patient lookup for Live Now Longevity clinic
homepage: https://github.com/drcharleskamen-png/oc-plugin-livenow
permissions:                 # declared upfront; install prompts for these
  network:
    - api.livenowlongevity.com
  fs_read: ["~/livenow-data/"]
  fs_write: ["~/livenow-data/cache/"]
  secrets: [LIVENOW_API_KEY]  # requested from keyring on first use
extension:                    # exactly one ExtensionConfig stanza
  type: stdio
  name: livenow-booking
  cmd: python
  args: ["-m", "oc_plugin_livenow"]
  env_keys: [LIVENOW_API_KEY]
  timeout: 300
prompt: |                     # optional system-prompt snippet injected when active
  When booking a peptide appointment, confirm dosage + provider first.
tools:                        # optional explicit tool declarations (for SDK)
  - name: book_appointment
    description: Book a clinic appointment
lazy: true                    # scope tools into context only when task matches (§5)
```

Loader: `PluginLoader` reads `octocode-plugin.yaml`, validates `api_version`,
checks the signature (§4), and feeds `extension:` into the existing
`ExtensionConfig` path. **Zero changes to the extension runtime** — a plugin is
just a versioned, signed, permission-declared wrapper around what already loads.

## 4. Marketplace + signing

- **Registry:** a git repo (or GitHub repo as registry) of plugin index files.
  v1.0 = a static index in `drcharleskamen-png/oc-plugins` (signed tags). No
  server to operate. v2 = HTTP registry.
- **Package format:** `.ocpkg` = tarball of plugin dir + a detached
  minisign/cosign signature over the manifest hash. Loader verifies signature
  against a trusted publisher set in config before install.
- **CLI:**
  ```
  goose plugin install <name>[@version]   # fetch .ocpkg, verify sig, write to ~/.config/goose/plugins/, enable
  goose plugin list                       # installed + available, versions, signature status
  goose plugin remove <name>
  goose plugin update [name]              # check registry, bump if signed-newer
  goose plugin new <name>                 # scaffold a plugin from template (SDK)
  ```
- **Trust model:** three tiers, matching spec §2.8 (per-tool sandboxing levels).
  - `trusted` (bundled, signed by OctoCode): in-process `Builtin`/`Platform`.
  - `signed` (third-party, signed + in publisher set): `Stdio`/`InlinePython`
    subprocess, declared permissions enforced.
  - `untrusted` (unsigned, explicit `--insecure` install): `Stdio` inside the
    Docker sandbox (spec §8 single-sandbox rule), no network unless granted.

## 5. SDK (the part that earns the "SDK" name)

Two thin helpers, one per ecosystem. Neither is required to author a plugin
(any MCP server works), but both remove boilerplate.

- **Rust** (`crates/goose-plugin-sdk`, new): a `Plugin` trait + proc macro that
  generates the manifest, an MCP server over stdio, and the permission
  declarations from annotated `#[tool]` fn items. Reuses `goose-mcp`'s
  transport.
- **Python** (`goose plugin new --python` template): a `pyproject`-based
  scaffold using the official MCP Python SDK + an `octocode.permissions`
  decorator so `permissions:` in the manifest stays in sync with code.

Both SDKs emit the same `octocode-plugin.yaml`, so the marketplace is
language-agnostic.

## 6. Lazy tool loading (Part 4 integration — load-bearing)

The `available_tools` allowlist is currently static. For v1.0 we add a
`lazy: true` plugin mode where the **tool schemas** are not injected into the
system prompt until a task router (reusing the spine-item-2 router) detects a
match against the plugin's declared tool purposes. Concretely:

- Plugin manifest declares tool `purpose` strings (cheap, short).
- The router sees the user turn + indexed purposes, decides which lazy plugins
  to "activate" for that turn, and only then does the agent fetch + inject those
  tool schemas.
- This is the mechanism that makes 200+ ecosystem tools affordable (spec §8):
  the model never sees schemas it doesn't need this turn.

Net token cost: prefix cache stays hot (system prompt + active-tool-schemas
deterministic for a given task class); per-turn schema bloat drops to the
~5–15 tools the task actually needs.

## 7. Sandbox decision (recommend, escalate)

Spec §8 fixes v1 sandbox = Docker. Mapping that onto plugin trust:

- `trusted`/`signed` Stdio plugins: run as normal subprocess (process
  isolation + declared permissions). Acceptable for v1.
- `untrusted` plugins: Stdio command is invoked *inside* the Docker sandbox
  profile. No host FS/network beyond what `permissions:` grants.
- **WASM sandbox (spec §2.8) deferred to post-v1.** Reason: WASM module loader
  + capability API is a project of its own; Docker-for-untrusted + process-
  isolation-for-signed covers the v1 threat model without it. Escalation point:
  if Charles wants browser-extension-tier sandboxing in v1, this changes scope.

## 8. Phasing inside v1.0 (smallest viable)

1. **Manifest + loader** (§3) — formalize `ExtensionConfig` wrapper. Smallest
   unit; unblocks everything else.
2. **CLI `plugin install/list/remove`** (§4) against a static git registry +
   minisign verify.
3. **Rust SDK crate + `goose plugin new`** (§5) — one reference plugin shipped
   (e.g. wraps an existing goose-mcp tool as a plugin to prove the loop).
4. **Lazy tool loading** (§6) — wires into the spine-item-2 router.
5. Marketplace = the registry repo going public + signed first-party plugins.

Each phase ships independently and is independently useful.

## 9. Open questions for Charles (escalate before implementing)

1. **Registry host:** static git repo (`drcharleskamen-png/oc-plugins`) vs HTTP
   registry vs piggyback on an existing MCP registry? Git = zero ops, fine for
   v1.
2. **Signing scheme:** minisign (simple, single-publisher-friendly) vs
   sigstore/cosign (keyless, OIDC, heavier)? Affects SDK + release pipeline.
3. **SDK languages for v1:** Rust + Python both, or Python first (largest MCP
   author base) + Rust later?
4. **WASM in v1 or post-v1?** (§7) — I recommend post-v1; confirm.
5. **License implications:** plugin manifest carries a `license` field; does an
   OctoCode plugin registry need a contributor CLA / license grant? Tied to
   the §10 licensing open question (Apache base vs AGPL/SaaS).
6. **Trademark:** "octocode-plugin" naming assumes "OctoCode" clears (§10). If
   the rename is deferred, the manifest can be `goose-plugin.yaml` for now and
   renamed in tree later — confirm preference.

## 10. What this proposal does NOT do (scope discipline)

- No new plugin runtime. MCP is the runtime.
- No WASM in v1.
- No HTTP registry server in v1.
- No multi-tenant marketplace (ratings, search, payments) — that's §3.18/v2.0+.
- No change to the existing extension config format (manifest is additive).
