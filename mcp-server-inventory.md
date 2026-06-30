# MCP server inventory

Inventory for MCP documentation cleanup and future live checks. Generated from `documentation/static/servers.json` and `documentation/docs/mcp/*`.

- Catalog entries: 57
- MCP doc pages: 74
- MCP doc pages without catalog entry: 18

## Live check levels

- `local`: built-in/platform extension. Check with repo tests or local goose code; no external package needed.
- `stdio`: stdio package with no required secrets in catalog. Good candidate for automated smoke checks: install/launch, initialize MCP, list tools, stop.
- `stdio+secret`: stdio package needs API keys or service credentials. We can check package existence and maybe startup error quality, but real tool calls need secrets.
- `remote`: hosted Streamable HTTP/OAuth/local HTTP endpoint. We can check catalog shape and maybe endpoint reachability, but real MCP auth usually needs OAuth or a running local app.
- `archived`: unlisted/archived doc. Decide whether to keep, hide, or mark archived before live checking.

## Suggested live-check order

1. `local` entries: verify against code/tests.
2. `stdio` entries: run MCP initialize/list-tools smoke checks in a sandboxed temp config.
3. `stdio+secret` entries: verify package resolution and required env var docs; run real checks only when credentials are available.
4. `remote` entries: verify URL/type/auth metadata; run live checks only for public unauthenticated endpoints or with OAuth/API access.
5. `archived` pages: decide policy first.

## Smoke check results

Run: `mcp-smoke-results.md`

Date: 2026-06-29. Scope was catalog-backed stdio servers with no required secrets and a non-empty command. The check launched each command, sent MCP `initialize`, sent `tools/list`, then stopped.

- Checked: 22
- Passed: 22
- Failed: 0

Removed after smoke check: `i-ching` and `sugar`.

Removed after placeholder-command review: `cognee-mcp`.

Removed after placeholder-command review: `mbot-mcp`.

## Catalog-backed servers

| ID | Name | Doc | Check | Required secrets | Command or URL |
|---|---|---|---|---|---|
| `agentql-mcp` | AgentQL | `agentql-mcp.md` | stdio+secret | AGENTQL_API_KEY | `npx -y agentql-mcp` |
| `alby-mcp` | Alby | `alby-mcp.md` | stdio+secret | NWC_CONNECTION_STRING | `npx -y @getalby/mcp` |
| `apify` | Apify | `apify-mcp.md` | stdio+secret | APIFY_TOKEN | `npx -y @apify/actors-mcp-server` |
| `asana-mcp` | Asana | `asana-mcp.md` | stdio+secret | ASANA_ACCESS_TOKEN | `npx -y @roychri/mcp-server-asana` |
| `autovisualiser` | Auto Visualiser | `autovisualiser-mcp.md` | local |  |  |
| `beads` | Beads | `beads-mcp.md` | stdio |  | `uvx beads-mcp` |
| `blender-mcp` | Blender | `blender-mcp.md` | stdio |  | `uvx blender-mcp` |
| `browserbase-mcp` | Browserbase | `browserbase-mcp.md` | stdio+secret | BROWSERBASE_API_KEY | `npx -y @browserbasehq/mcp` |
| `chrome-devtools-mcp` | Chrome DevTools | `chrome-devtools-mcp.md` | stdio |  | `npx -y chrome-devtools-mcp@latest` |
| `cloudinary-asset-management-mcp` | Cloudinary Asset Management | `cloudinary-asset-management-mcp.md` | stdio+secret | CLOUDINARY_URL | `npx -y --package @cloudinary/asset-management -- mcp start` |
| `computercontroller` | Computer Controller | `computer-controller-mcp.md` | local |  |  |
| `container-use` | Container Use | `container-use-mcp.md` | stdio |  | `npx -y mcp-remote https://container-use.com/mcp` |
| `context7` | Context7 | `context7-mcp.mdx` | stdio |  | `npx -y @upstash/context7-mcp` |
| `council-of-mine` | Council of Mine | `council-of-mine-mcp.md` | stdio |  | `uvx --from git+https://github.com/block/mcp-council-of-mine mcp_council_of_mine` |
| `datahub-mcp` | DataHub | `datahub-mcp.mdx` | stdio+secret | DATAHUB_GMS_URL, DATAHUB_GMS_TOKEN | `uvx mcp-server-datahub@latest` |
| `code_execution` | Code Mode | `code-mode-mcp.md` | local |  |  |
| `developer` | Developer | `developer-mcp.md` | local |  |  |
| `dev.to` | Dev.to | `dev.to-mcp.md` | remote |  | `http://localhost:3000/mcp` |
| `elevenlabs-mcp` | ElevenLabs | `elevenlabs-mcp.md` | stdio+secret | ELEVENLABS_API_KEY | `uvx elevenlabs-mcp` |
| `exa` | Exa Search | `exa-mcp.md` | stdio+secret | EXA_API_KEY | `npx -y exa-mcp-server` |
| `excalidraw-mcp-app` | Excalidraw | missing | remote |  | `https://excalidraw-mcp-app.vercel.app/mcp` |
| `fetch` | Fetch | `fetch-mcp.md` | stdio |  | `uvx mcp-server-fetch` |
| `figma` | Figma | `figma-mcp.md` | remote |  | `http://127.0.0.1:3845/mcp` |
| `github-mcp` | GitHub | `github-mcp.md` | remote |  | `https://api.githubcopilot.com/mcp/` |
| `gitmcp` | GitMCP | `gitmcp-mcp.md` | stdio |  | `npx -y mcp-remote https://gitmcp.io/docs` |
| `goose-docs` | goose Docs | `goose-docs-mcp.md` | stdio |  | `npx mcp-remote https://block.gitmcp.io/goose/` |
| `gotoHuman-mcp` | gotoHuman MCP Server | `gotohuman-mcp.md` | stdio |  | `npx -y @gotohuman/mcp-server` |
| `jetbrains` | JetBrains | `jetbrains-mcp.md` | stdio |  |  |
| `knowledge_graph_memory` | Knowledge Graph Memory | `knowledge-graph-mcp.md` | stdio |  | `npx -y @modelcontextprotocol/server-memory` |
| `linux-mcp-server` | Linux MCP Server | `linux-mcp-server-mcp.md` | stdio |  | `uvx linux-mcp-server` |
| `memory` | Memory | `memory-mcp.md` | local |  |  |
| `mongodb` | MongoDB | `mongodb-mcp.md` | stdio |  | `npx -y mongodb-mcp-server --connectionString mongodb://localhost:27017` |
| `nano-banana-mcp` | Nano Banana | `nano-banana-mcp.md` | stdio+secret | GEMINI_API_KEY | `npx nano-banana-mcp` |
| `neon` | Neon | `neon-mcp.md` | remote |  | `https://mcp.neon.tech/mcp` |
| `netlify-mcp` | Netlify | `netlify-mcp.md` | stdio+secret | NETLIFY_ACCESS_TOKEN | `npx -y @netlify/mcp` |
| `nostrbook-mcp` | NostrBook | `nostrbook-mcp.md` | stdio |  | `npx -y @nostrbook/mcp@latest` |
| `openmetadata` | OpenMetadata | `openmetadata-mcp.md` | stdio+secret | AUTH_HEADER | `npx -y mcp-remote http://localhost:8585/mcp --auth-server-url=http://localhost:8585/mcp --client-id=openmetadata --verbose --clean --header Authorization:${AUTH_HEADER}` |
| `ophis` | Ophis | `ophis-mcp.md` | remote |  | `https://mcp.ophis.fi/mcp` |
| `pdf_read` | PDF Reader | `pdf-mcp.md` | stdio |  | `uvx mcp-read-pdf` |
| `pieces` | Pieces | `pieces-mcp.md` | stdio |  | `uvx --from pieces-cli pieces --ignore-onboarding mcp start` |
| `playwright` | Playwright | `playwright-mcp.md` | stdio |  | `npx @playwright/mcp@latest` |
| `prompts-chat-mcp` | prompts.chat | `prompts-chat-mcp.md` | stdio |  | `npx -y @fkadev/prompts.chat-mcp@latest` |
| `reddit-mcp` | Reddit | `reddit-mcp.md` | stdio+secret | REDDIT_CLIENT_ID, REDDIT_CLIENT_SECRET | `npx -y reddit-mcp` |
| `rendex-mcp` | Rendex | `rendex-mcp.md` | remote |  | `https://mcp.rendex.dev/mcp` |
| `repomix-mcp` | Repomix | `repomix-mcp.md` | stdio |  | `npx -y repomix-mcp` |
| `rube-mcp` | Rube | `rube-mcp.md` | remote |  | `https://rube.app/mcp` |
| `selenium-mcp` | Selenium | `selenium-mcp.md` | stdio |  | `npx -y @angiejones/mcp-selenium` |
| `square-mcp` | Square | `square-mcp.md` | stdio+secret | ACCESS_TOKEN, SANDBOX | `npx square-mcp-server start` |
| `summon` | Summon | `summon-mcp.md` | local |  |  |
| `supabase` | Supabase | `supabase-mcp.md` | remote |  | `https://mcp.supabase.com/mcp` |
| `tavily` | Tavily Web Search | `tavily-mcp.md` | stdio+secret | TAVILY_API_KEY | `npx -y tavily-mcp` |
| `tom` | Top of Mind | `tom-mcp.md` | local |  |  |
| `tutorial-mcp` | Tutorial | `tutorial-mcp.md` | local |  |  |
| `youtube-transcript-mcp` | YouTube Transcript | `youtube-transcript-mcp.md` | stdio |  | `uvx --from git+https://github.com/jkawamoto/mcp-youtube-transcript mcp-youtube-transcript` |
| `neighborhood` | Neighborhood | `neighborhood-mcp.md` | remote |  | `https://connect.squareup.com/v2/mcp/neighborhood` |
| `cash-app` | Cash App | `cash-app-mcp.md` | remote |  | `https://connect.squareup.com/v2/mcp/cash-app` |
| `scholar-sidekick` | Scholar Sidekick | `scholar-sidekick-mcp.md` | stdio |  | `npx -y scholar-sidekick-mcp@latest` |

## Doc pages without catalog entry

| Doc | Title | Unlisted | Initial policy question |
|---|---|---|---|
| `apps-mcp.md` | Apps Extension | no | Should this be added to catalog, or is it a platform/internal page? |
| `brave-mcp.md` | Brave Search Extension | yes | Archived/unlisted: keep, hide, or refresh? |
| `chatrecall-mcp.md` | Chat Recall Extension | no | Should this be added to catalog, or is it a platform/internal page? |
| `cloudflare-mcp.md` | Cloudflare Extension | yes | Archived/unlisted: keep, hide, or refresh? |
| `excalidraw-mcp.md` | Excalidraw Extension | no | Should this be added to catalog, or is it a platform/internal page? |
| `extension-manager-mcp.md` | Extension Manager | no | Should this be added to catalog, or is it a platform/internal page? |
| `firecrawl-mcp.md` | Firecrawl Extension | no | Should this be added to catalog, or is it a platform/internal page? |
| `google-drive-mcp.md` | Google Drive Extension | yes | Archived/unlisted: keep, hide, or refresh? |
| `google-maps-mcp.md` | Google Maps Extension | yes | Archived/unlisted: keep, hide, or refresh? |
| `kiwi-flight-search.md` | Kiwi Flight Search Extension | yes | Archived/unlisted: keep, hide, or refresh? |
| `postgres-mcp.md` | PostgreSQL Extension | yes | Archived/unlisted: keep, hide, or refresh? |
| `puppeteer-mcp.md` | Puppeteer Extension | yes | Archived/unlisted: keep, hide, or refresh? |
| `skills-mcp.md` | Skills Extension | no | Should this be added to catalog, or is it a platform/internal page? |
| `speech-mcp.md` | Speech Extension | yes | Archived/unlisted: keep, hide, or refresh? |
| `todo-mcp.md` | Todo Extension | no | Should this be added to catalog, or is it a platform/internal page? |
| `vercel-mcp.md` | Vercel Extension | no | Should this be added to catalog, or is it a platform/internal page? |
| `vmware-aiops-mcp.md` | VMware AIops Extension | no | Should this be added to catalog, or is it a platform/internal page? |
| `vs-code-mcp.md` | VS Code Extension | yes | Archived/unlisted: keep, hide, or refresh? |

## Notes before live checks

- Do not run third-party commands with real credentials until we choose which servers to test and where credentials should come from.
- Prefer MCP initialize/list-tools smoke checks over invoking tools that mutate external systems.
- Some stdio packages may open browsers, spawn long-running local services, or require desktop apps. Those should be opt-in after command inspection.
- External package availability changes over time; live check results should include date and exact command/version where possible.
