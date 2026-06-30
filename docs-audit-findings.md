# Documentation audit findings

Read-only audit of documentation pages in this worktree. Scope was tracked docs under `documentation/docs/**/*.md(x)` plus `documentation/src/pages/*.md(x)`, excluding blog posts.

Total pages checked: 180. No source files were edited during the audit.

Related trackers:

- `mcp-server-inventory.md`: catalog/doc inventory plus proposed live-check levels.
- `mcp-smoke-results.md`: automated MCP initialize/list-tools smoke results for no-secret stdio catalog entries.

## Fix progress

### Priority 1: rendering/build-risk issues

Started. Fixed in this worktree:

- `documentation/docs/mcp/rube-mcp.md`: closed the Quick Install admonition and removed an unused import.
- `documentation/docs/mcp/netlify-mcp.md`: closed the CLI note before the success headings so they do not render inside the admonition.
- `documentation/docs/mcp/supabase-mcp.md`: replaced nested triple backticks inside example output with tildes.
- `documentation/docs/mcp/speech-mcp.md`: removed the visible maintenance note from the page body.
- `documentation/docs/mcp/nostrbook-mcp.md`: closed the CLI note before the generated answer heading.
- `documentation/docs/mcp/knowledge-graph-mcp.md`: unindented headings/fences and split nested code fences into safe Markdown blocks.
- `documentation/docs/tutorials/subagents.md`: replaced generated hashed frontmatter image with the source image path and removed the hidden preload workaround.
- `documentation/src/pages/grants.md`: removed unused import, replaced linkless `Link` section labels with headings, and pointed OG/Twitter images at the stable source image.
- `documentation/src/pages/markdown-page.md`: removed the default Docusaurus sample page.
- `documentation/docs/mcp/i-ching-mcp.md`: removed page and catalog entry.
- `documentation/docs/mcp/sugar-mcp.mdx`: removed page and catalog entry.
- `documentation/docs/mcp/cognee-mcp.md`: removed page and placeholder-path catalog entry.
- `documentation/docs/tutorials/advanced-cognee-usage.md`: removed tutorial because it depended on the removed Cognee extension page.
- `documentation/docs/mcp/mbot-mcp.md`: removed page and placeholder-path catalog entry.

### CLI commands reference

Started. Added reference sections for the main current commands the docs were missing and that we want to keep visible for now:

- `serve`
- `skills`
- `term`
- `local-models`
- `review`

Left more experimental commands out of the guide for now.

### Platform extension split

Started. Fixed in this worktree:

- `documentation/docs/getting-started/using-extensions.md`: added Analyze and Skills to the platform extension list, and narrowed Summon/Code Mode wording.
- `documentation/docs/mcp/developer-mcp.md`: removed stale Analyze/screenshot/image-processor claims and listed the current Developer tools.
- `documentation/docs/guides/codebase-analysis.md`: documented Analyze as a separate platform extension instead of part of Developer, updated the source link, removed stale `.gooseignore` advice, and fixed the large-output threshold.
- `documentation/docs/guides/context-engineering/using-skills.md`: changed skill loading from Summon to the Skills platform extension and added current CLI loading examples.
- `documentation/docs/mcp/summon-mcp.md`: changed Summon wording and example to recipes, agents, subrecipes, and delegation instead of skill loading.
- `documentation/docs/guides/sessions/smart-context-management.md`: changed manual compaction docs from `/summarize` to `/compact`, documented `/summarize` as a deprecated alias, and fixed tool-output cutoff wording.
- `documentation/docs/guides/context-engineering/prompt-templates.md`: added missing `tiny_model_system.md` and `session_name.md` templates.

### MCP live-check progress

Started. Created `scripts/mcp-smoke-check.js` and ran it against catalog-backed stdio servers that do not need required secrets.

- Checked: 22
- Passed: 22
- Failed: 0

Removed failed entries: `i-ching` and `sugar`. Details are in `mcp-smoke-results.md`.

### MCP catalog/docs normalization

Started. Fixed in this worktree:

- Normalized catalog-backed MCP page ids in quick-install links and installer components.
- Replaced `Streaming HTTP` wording with `Streamable HTTP` across MCP setup pages.
- Aligned obvious command drift for Browserbase, NostrBook, Reddit, Repomix, MongoDB, and Selenium.
- Removed unsupported `GooseDesktopInstaller` props such as `timeout`, `cliCommand`, and `note`.
- Updated the MCP template so new pages do not copy unsupported Desktop installer props.

### Stale core docs

Started. Removed in this worktree:

- `documentation/docs/guides/sandbox.md`
- `documentation/docs/guides/enhanced-code-editing.md`
- `documentation/docs/guides/context-engineering/using-gooseignore.md`
- `documentation/docs/guides/managing-projects.md`

Also removed or rewired links to those pages from guides, MCP docs, the context-engineering index, the security index, redirects, and blog posts.

## Repeated fix buckets

- MCP catalog drift: page `extensionId`/deeplink ids often differ from `documentation/static/servers.json`; commands, env vars, and catalog entries often disagree.
- Installer component drift: pages pass props such as `timeout`, `cliCommand`, and JSX `infoNote` where the components do not consume or type them.
- Streamable HTTP wording drift: many pages say `Streaming HTTP`; current UI/component wording is mostly `Streamable HTTP`.
- Old CLI configure transcripts: many archived/unlisted MCP pages still show optional description prompts; current CLI prompts directly for a required description.
- Markdown/MDX rendering risks: unclosed admonitions, nested code fences, headings inside admonitions, generated hashed image URLs, unused imports, and default sample pages.
- Skills/Summon split: fixed in the main Skills and Summon docs. Remaining related pages such as subagents/subrecipes may still need a consistency pass.
- Archived/unlisted pages: several pages are hidden or archived but still contain active install instructions or stale examples.
- External MCP behavior: third-party server tool names/capabilities were generally not repo-checkable unless mirrored by local catalog, docs, tests, or code.

## Product and guide pages

- `experimental/index.md`: mobile app remote access mention is stale; mobile tunneling was retired.
- `experimental/remote-access/index.md`: says remote access supports multiple platforms, but local code only registers Telegram.
- `remote-access/telegram-gateway.md`: Desktop setup UI is stale; Gateway settings UI was not found. CLI status/stop controls also look unreliable for an already-running gateway.
- `getting-started/providers.md`: `o1-preview` unsupported claim is stale; code only hard-blocks `o1-mini`. Bedrock prompt caching requires `BEDROCK_ENABLE_CACHING=true`. OpenAI row omits `OPENAI_BASE_URL`, `OPENAI_BASE_PATH`, and `OPENAI_TIMEOUT`.
- `getting-started/using-extensions.md`: fixed platform extension list for `Analyze` and `Skills`; Code Mode wording now notes build availability.
- `goose-architecture/error-handling.md`: references `AgentError`; current code uses RMCP `ErrorData`/`ToolResult`.
- `goose-architecture/extensions-design.md`: old `Extension` trait design does not match current MCP-client/config extension system.
- `guides/acp-clients.md`: TUI permission dialog and `Tab` shortcut details are stale.
- `guides/acp-providers.md`: omits `copilot-acp`; default model tables are stale; “No session fork/resume” is misleading.
- `guides/allowlist.md`: says Desktop+CLI but evidence points to Desktop only; matching appears prefix-based rather than exact; fetch errors fail open; logs are Electron/UI-side.
- `guides/cli-providers.md`: Claude Code/Codex now pass MCP config to underlying CLI; default model/settings are stale; `CODEX_ENABLE_SKILLS` not found.
- `guides/codebase-analysis.md`: fixed Analyze platform-extension split, current source link, `.gitignore` behavior, and large-output threshold.
- `guides/config-files.md`: flat root `GOOSE_PROVIDER`/`GOOSE_MODEL` config is legacy; theme default is `ansi`; extension types incomplete.
- `guides/creating-plans.md`: mostly current.
- `guides/custom-agents.md`: mostly current; example `gpt-5.5` not repo-checkable.
- `guides/hooks.md`: mostly current, but omits blocking via exit code `2` / `{"decision":"block"}`.
- `guides/plugins.md`: says plugins provide only skills/hooks; current plugins can also provide MCP servers.
- `guides/prompt-templates.md`: fixed; page now lists all 10 templates in the registry.
- `guides/slash-commands.md`: “only one parameter” is stale; current parsing supports multiple required params and optional flags. Slash command sources are broader than described.
- `guides/subagents.mdx`: stale mode restrictions; timeout behavior wording wrong; “Return Mode Control” setting not found; typo `.~/.config`; duplicate resource card.
- `guides/using-goosehints.md`: Developer extension not required; default context filenames are `.goosehints`, then `AGENTS.md`; priority/config nuance needs tightening.
- `guides/using-gooseignore.md`: appears broadly stale; no `.gooseignore` implementation found in `crates/`; tree uses `.gitignore`.
- `guides/using-persistent-instructions.md`: mostly current.
- `guides/using-skills.md`: fixed; Skills is documented as a separate platform extension.
- `guides/desktop-navigation.md`: sidebar and nav customization claims are stale; current nav is fixed left with remembered open/close.
- `guides/enhanced-code-editing.md`: feature appears stale; no `str_replace` or `GOOSE_EDITOR_*` refs found; current `edit` is simple replacement.
- `guides/environment-variables.md`: `GOOSE_SHELL` fallback not `$SHELL`; search path default incomplete; enhanced-code vars stale; `GOOSE_CLI_TOOL_PARAMS_MAX_LENGTH` example typo/stale.
- `guides/file-management.md`: search behavior differs; searches cwd normally and user dirs as fallback; highlighted results not seen.
- `guides/goose-cli-commands.md`: partially fixed for `serve`, `skills`, `term`, `local-models`, and `review`; more experimental commands intentionally left out for now. Remaining known drift: `mcp <name>` description stale; theme and slash command lists incomplete; editor precedence includes `VISUAL`/`EDITOR`.
- `guides/handling-llm-rate-limits...`: provider switch-on-rate-limit behavior is external/provider-side, not goose-side.
- `guides/interactive-chat/index.mdx`: one blog link `2026-01-06-mcp-apps` not found.
- `guides/interactive-chat/mcp-ui.md`: Apps page filtering looks narrower than “apps from enabled MCP Apps extensions.”
- `guides/logs.md`: command history path is state dir, not config dir; “never sent to external servers” is too absolute because tracing can send data.
- `guides/managing-projects.md`: project path is platform-specific data dir, not fixed `~/.local/share`; Desktop support planned claim not repo-checkable.
- `guides/adjust-tool-output.md`: CLI path stale; setting is `Tool Output`; label is `All (default)`.
- `guides/code-mode.md`: tool exposure depends on `CODE_MODE_TOOL_DISCLOSURE`; not always 3 meta-tools; “every request writes JS” too absolute.
- `guides/goose-permissions.md`: mostly OK; Claude Code permission integration also `smart_approve`; CLI provider framing omits deprecation nuance.
- `guides/tool-permissions.md`: Developer tools listed stale; no screen capture/image processor; Desktop permission editing requires active session.
- `guides/mcp-elicitation.md`: current.
- `guides/mcp-roots.md`: current, minor Desktop directory-picker wording.
- `guides/mcp-sampling.md`: mostly OK; core route uses global model config while Desktop uses session model.
- `guides/multi-model/index.mdx`: planner config supported; “dynamic, context-aware switching” not found.
- `guides/recipes/index.mdx`: `goals` is not a recipe schema field.
- `guides/recipe-reference.md`: contradictory `.yml`; current extensions are `yaml/json`; search paths incomplete; `settings.max_turns` wording overstates main-agent behavior; validation rules stale.
- `guides/session-recipes.md`: `version` not required; search paths incomplete; extension existence validation not found; scheduler accepts 5/6-field cron, not 7.
- `guides/storing-recipes.md`: Desktop Recipe Library does discover local recipes; paths incomplete with `.agents/recipes`.
- `guides/subrecipes.md`: fields incomplete; values can be overridden by delegate params; nested subrecipe validation not found.
- `guides/remote-goose-server.md`: Desktop accepts HTTP/HTTPS; doc says remote HTTP refused and TLS required; fingerprint is optional/TOFU but doc treats it as required.
- `guides/running-tasks.md`: `--with-remote-extension` stale; `--with-builtin "developer,git"` example references no found `git` built-in.
- `guides/sandbox.md`: largely stale; no `GOOSE_SANDBOX`, `sandbox-exec`, sandbox index, blocked list, or proxy integration found.
- `guides/security/adversary-mode.md`: mostly OK, but “cannot retry” too strong; “before each tool call” too broad.
- `guides/security/classification-api-spec.md`: mostly OK; URL wording too narrow; classifier model can come via `SECURITY_ML_MODEL_MAPPING`.
- `guides/security/prompt-injection-detection.md`: scans only shell tool calls; threshold direction inverted; ML prompt vs command classifiers separate; UI label differs.
- `guides/sessions/in-session-actions.md`: fork name is `(copy)`, not `(edited)`; Clear All appears only with more than one queued message; bottom toolbar mode switcher stale.
- `guides/sessions/session-management.md`: Desktop search cap questionable; CLI session import exists despite Desktop-only wording.
- `guides/smart-context-management.md`: fixed; uses `/compact`, notes `/summarize` as deprecated alias, and describes computed tool-output cutoff.
- `guides/tanzu-ai-services.md`: provider wiring current; external Tanzu claims not checked.
- `guides/tanzu-cli-testing-guide.md`: stale branch note; endpoint examples are environment-specific; session IDs in terminal docs old.
- `guides/terminal-integration.md`: session ID examples old `YYYYMMDD_HHMMSS`; current format is `YYYYMMDD_<counter>`.
- `guides/tips.md`: mostly OK; small typo “tool” -> “tools”.
- `guides/updating-goose.md`: OK.
- `guides/usage-data.md`: collected-data list overstates PostHog path; error telemetry disabled in `emit_error`; tool usage counts are tracing/OTel, not PostHog `session_started`.
- `mcp/_template_.mdx`: `extensionId` passed to `GooseBuiltinInstaller` but component does not accept it; `CLIExtensionInstructions.infoNote` typed string but template passes JSX.

## MCP pages

- `agentql-mcp.md`: time-sensitive examples stale.
- `alby-mcp.md`: mostly OK; unused `PanelLeft`; live values dynamic.
- `apify-mcp.md`: local quick-install deeplink inconsistent with catalog; command should likely use `npx -y`.
- `apps-mcp.md`: macOS path stale; code uses `Paths::in_data_dir("apps")`; “no external dependencies” conflicts with allowed external fonts/icons/CSS.
- `asana-mcp.md`: OK locally.
- `autovisualiser-mcp.md`: typo in CLI example; catalog says MCP-UI while implementation/page says MCP Apps.
- `beads-mcp.md`: locally consistent.
- `blender-mcp.md`: installer id `blender` vs catalog `blender-mcp`; duplicate example intro.
- `brave-mcp.md`: leftover “Server moved”; manual CLI flow stale.
- `browserbase-mcp.md`: command/env/id mismatches against catalog.
- `cash-app-mcp.md`: locally consistent.
- `chatrecall-mcp.md`: “search across all sessions” too broad; excludes current session and filters session types; “Load summaries” returns first/last 3 messages, not generated summaries.
- `chrome-devtools-mcp.md`: command OK; id `chrome-devtools` vs catalog `chrome-devtools-mcp`.
- `cloudflare-mcp.md`: endpoint mismatch `/mcp` vs `/sse`; `npx mcp-remote` missing `-y`.
- `cloudinary-asset-management-mcp.md`: command/env OK; id `cloudinary` vs catalog `cloudinary-asset-management-mcp`.
- `code-mode-mcp.md`: mostly current; pctx GitHub link differs from current guide.
- `cognee-mcp.md`: removed.
- `computer-controller-mcp.md`: setup current; example uses stale `web_search`; current tool is `web_scrape`.
- `container-use-mcp.md`: quick install defaults local while catalog defaults remote; unsupported `GooseDesktopInstaller` props.
- `context7-mcp.mdx`: OK; unsupported installer props.
- `council-of-mine-mcp.md`: OK locally.
- `datahub-mcp.mdx`: mostly OK locally.
- `dev.to-mcp.md`: installer id `dev-to` vs catalog `dev.to`; “Streaming HTTP” wording; catalog type spelling question.
- `developer-mcp.md`: fixed current Developer tool list and moved Analyze wording out of Developer.
- `elevenlabs-mcp.md`: id/env formatting inconsistencies; broader capabilities not catalog-backed.
- `exa-mcp.md`: OK locally.
- `excalidraw-mcp.md`: catalog description truncated; catalog id would generate wrong detail route; quick install outdated.
- `extension-manager-mcp.md`: CLI sample success text stale; “unused extensions” suggestion overstates local evidence.
- `fetch-mcp.md`: current locally; external claims not repo-checkable.
- `figma-mcp.md`: “Streaming” wording; unused imports.
- `firecrawl-mcp.md`: OK locally.
- `github-mcp.md`: id mismatch; “Streaming” wording.
- `gitmcp-mcp.md`: quick install omits `-y`; inconsistent ids/names.
- `google-drive-mcp.md`: CLI shell snippets broken; stale configure transcript; archived page still active-looking; no catalog entry.
- `google-maps-mcp.md`: unused import; stale configure transcript; transcript says `Added github extension`; no catalog entry.
- `goose-docs-mcp.md`: catalog OK; unsupported `cliCommand`/`note` props.
- `gotohuman-mcp.md`: catalog lacks required API key; id mismatch; account-specific links.
- `i-ching-mcp.md`: removed.
- `jetbrains-mcp.md`: catalog install note conflicts with page; prompt-library metadata stale; recipe generator marks builtin though catalog says non-builtin.
- `kiwi-flight-search.md`: expected `kiwi-flight-search-mcp.md` path missing; “Server moved” but still points to old Kiwi endpoint; missing CLI description prop; no catalog entry.
- `knowledge-graph-mcp.md`: mostly OK; large indented sections may render as code.
- `linux-mcp-server-mcp.md`: mostly OK; unused imports.
- `mbot-mcp.md`: removed.
- `memory-mcp.md`: “loads all saved memories” too broad; trigger-word table not code-backed.
- `mongodb-mcp.md`: mostly OK; manual deeplink uses unencoded Mongo URL.
- `nano-banana-mcp.md`: id mismatch.
- `neighborhood-mcp.md`: OK locally; catalog link empty.
- `neon-mcp.md`: local Desktop install passes literal `<YOUR_NEON_API_KEY>` as arg; unused import.
- `netlify-mcp.md`: id mismatch; catalog env var missing from page; unclosed admonition/headings inside note.
- `nostrbook-mcp.md`: id/name mismatch; `@latest` mismatch; heading inside note.
- `openmetadata-mcp.md`: mostly OK; typo “would an”.
- `ophis-mcp.md`: OK locally.
- `pdf-mcp.md`: mostly OK locally.
- `pieces-mcp.md`: naming inconsistency.
- `playwright-mcp.md`: catalog command omits `-y`.
- `postgres-mcp.md`: archived/unlisted, no catalog entry; stale configure flow; connection URL step misleading.
- `prompts-chat-mcp.md`: JSX passed to string-typed `infoNote`.
- `puppeteer-mcp.md`: archived/unlisted, no catalog entry; stale configure flow.
- `reddit-mcp.md`: command and required env vars differ from catalog; id mismatch.
- `rendex-mcp.md`: id mismatch; minor prompt wording.
- `repomix-mcp.md`: page command/id differ from catalog.
- `rube-mcp.md`: likely unclosed admonition; id mismatch; unused import; wording drift.
- `scholar-sidekick-mcp.md`: locally consistent.
- `selenium-mcp.md`: command/package mismatch fixed in catalog.
- `skills-mcp.md`: stale deprecation/version availability; skill search paths incomplete; no catalog entry.
- `speech-mcp.md`: unlisted page has visible maintenance note; imports after body text.
- `square-mcp.md`: id mismatch; `SANDBOX`/`PRODUCTION` env mismatch against catalog.
- `sugar-mcp.mdx`: removed.
- `summon-mcp.md`: fixed; Summon now describes recipes, agents, subrecipes, and delegation instead of skill loading.
- `supabase-mcp.md`: nested code fence likely breaks rendering.
- `tavily-mcp.md`: says `uv` needed though command is `npx`.
- `todo-mcp.md`: code-backed, but no catalog entry despite other platform extensions being listed.
- `tom-mcp.md`: mostly current; capitalization mismatch.
- `tutorial-mcp.md`: mostly current; forward-looking claim not repo-checkable.
- `vercel-mcp.md`: no catalog entry; inconsistent ids.
- `vmware-aiops-mcp.md`: no catalog entry; unsupported installer `timeout`; JSX `infoNote`.
- `vs-code-mcp.md`: unlisted without archived warning; command mismatch.
- `youtube-transcript-mcp.md`: mostly OK; id mismatch.

## Quickstart, troubleshooting, tutorials, and src pages

- `quickstart.md`: mostly current; several unused imports.
- `troubleshooting/desktop-startup-debugging.md`: current.
- `troubleshooting/diagnostics-and-reporting.md`: JSON skeleton omits current top-level fields; “Configuration Files” plural but implementation includes one `config.yaml`.
- `troubleshooting/index.mdx`: actual file is `.mdx`, not `.md`; links/components OK.
- `troubleshooting/known-issues.md`: stale macOS path; removal command path inconsistent; Windows Node path too narrow; airgapped `jbang` workaround questionable.
- `tutorials/advanced-cognee-usage.md`: removed.
- `tutorials/building-mcp-apps.md`: `_meta.ui.permissions` documented but Desktop drops permissions; CSP `frameDomains` default wording too exact.
- `tutorials/cicd.md`: uses generic `PROVIDER_API_KEY` not consumed by providers; should mention `goose run --quiet`.
- `tutorials/custom-extensions.md`: mostly current; minor `STDIO` label drift.
- `tutorials/goose-in-docker.md`: Docker keyring advice may be over-broad; `docker-compose` should probably be `docker compose`.
- `tutorials/headless-goose.md`: `GOOSE_CONTEXT_STRATEGY` not found; recipe `prompt` requirement stale because `instructions` also accepted; auto-summary wording should become auto-compaction wording.
- `tutorials/isolated-development-environments.md`: unused imports; automatic branch/container behavior belongs to external Container Use server.
- `tutorials/laminar.md`: mostly current; base OTLP endpoint enables traces, metrics, and logs, but page talks only about traces.
- `tutorials/langfuse.md`: prefer `LANGFUSE_PUBLIC_KEY` / `LANGFUSE_SECRET_KEY`; remote image URL has double slash.
- `tutorials/mlflow.md`: mostly current; same OTLP all-signals nuance.
- `tutorials/plan-feature-devcontainer-setup.md`: mostly OK; `docker-compose` examples could be modernized.
- `tutorials/playwright-skill.md`: Summon is default-enabled; exact external CLI commands/output should be verified separately.
- `tutorials/ralph-loop.md`: mostly OK; raw GitHub org/URLs should be normalized if canonical org changed.
- `tutorials/recipes-tutorial.md`: settings list misses `max_turns`.
- `tutorials/remotion-video-creation.md`: stale Summon-as-Skills flow.
- `tutorials/rpi.md`: mostly current.
- `tutorials/spraay-mcp.md`: extension behavior external-only.
- `tutorials/subagents.md`: generated hashed frontmatter image path; `child_process` incorrectly listed as npm dependency.
- `tutorials/subrecipes-in-parallel.md`: stale max workers; current default is 5 via `GOOSE_MAX_BACKGROUND_TASKS`; explicit async/load flow differs from page.
- `documentation/src/pages/community/data/README.md`: stale paths/schema; likely questionable as public page route.
- `documentation/src/pages/grants.md`: unused import; `<Link>` without `to`/`href`; hard-coded generated OG/Twitter image.
- `documentation/src/pages/markdown-page.md`: default Docusaurus sample page, likely should be removed or hidden.

## Suggested fix order

1. Fix rendering/build-risk issues: unclosed admonitions, nested fences, generated image URLs, sample page, visible maintenance notes.
2. Normalize MCP docs against `servers.json`: ids, commands, env vars, missing catalog entries, and `Streamable HTTP` wording.
3. Refresh stale core docs: sandbox, enhanced editing, `.gooseignore`, CLI commands, env vars, allowlist, navigation, remote access.
4. Refresh Skills/Summon/subagents/subrecipes pages together, since several pages share the same old mental model.
5. Decide policy for archived/unlisted MCP pages: update them, mark them clearly archived, or remove/hide active install flows.
