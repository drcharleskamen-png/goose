#!/usr/bin/env node

const fs = require("fs");
const { spawn } = require("child_process");

const SERVERS_PATH = "documentation/static/servers.json";
const OUT_PATH = "mcp-smoke-results.md";
const TIMEOUT_MS = Number(process.env.MCP_SMOKE_TIMEOUT_MS || 45000);
const PROTOCOL_VERSION = "2025-03-26";

function hasRequiredSecret(server) {
  return (server.environmentVariables || []).some((env) => env.required);
}

function isRemote(server) {
  return server.type === "streamable-http" || /^https?:\/\//.test(server.command || "");
}

function smokeCandidates(servers) {
  return servers
    .filter((server) => !server.is_builtin)
    .filter((server) => server.command && server.command.trim())
    .filter((server) => !hasRequiredSecret(server))
    .filter((server) => !isRemote(server));
}

function encodeMessage(message) {
  return `${JSON.stringify(message)}\n`;
}

function tryParseMessages(buffer) {
  const messages = [];

  while (true) {
    const headerEnd = buffer.indexOf("\r\n\r\n");
    if (headerEnd === -1) break;

    const header = buffer.slice(0, headerEnd).toString("utf8");
    const match = header.match(/Content-Length:\s*(\d+)/i);
    if (!match) {
      buffer = buffer.slice(headerEnd + 4);
      continue;
    }

    const length = Number(match[1]);
    const bodyStart = headerEnd + 4;
    const bodyEnd = bodyStart + length;
    if (buffer.length < bodyEnd) break;

    const body = buffer.slice(bodyStart, bodyEnd).toString("utf8");
    messages.push(JSON.parse(body));
    buffer = buffer.slice(bodyEnd);
  }

  return { messages, buffer };
}

function tryParseLineMessages(text) {
  const messages = [];
  const lines = text.split(/\r?\n/);
  const rest = lines.pop() || "";

  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed.startsWith("{")) continue;
    try {
      messages.push(JSON.parse(trimmed));
    } catch {
      // Ignore non-JSON log lines.
    }
  }

  return { messages, rest };
}

function waitForResponse(state, id) {
  const existing = state.messages.find((message) => message.id === id);
  if (existing) return Promise.resolve(existing);

  return new Promise((resolve) => {
    state.waiters.set(id, resolve);
  });
}

function pushMessages(state, messages) {
  for (const message of messages) {
    state.messages.push(message);
    if (message.id !== undefined && state.waiters.has(message.id)) {
      state.waiters.get(message.id)(message);
      state.waiters.delete(message.id);
    }
  }
}

function commandFor(server) {
  return server.command;
}

async function checkServer(server) {
  const command = commandFor(server);
  const state = {
    messages: [],
    waiters: new Map(),
    frameBuffer: Buffer.alloc(0),
    lineBuffer: "",
    stderr: "",
  };

  const child = spawn(command, {
    shell: true,
    stdio: ["pipe", "pipe", "pipe"],
    env: { ...process.env, NO_COLOR: "1" },
  });

  const startedAt = Date.now();
  let timedOut = false;

  const timer = setTimeout(() => {
    timedOut = true;
    child.kill("SIGTERM");
    setTimeout(() => child.kill("SIGKILL"), 2000).unref();
  }, TIMEOUT_MS);

  child.stdout.on("data", (chunk) => {
    state.frameBuffer = Buffer.concat([state.frameBuffer, chunk]);
    try {
      const parsed = tryParseMessages(state.frameBuffer);
      state.frameBuffer = parsed.buffer;
      pushMessages(state, parsed.messages);
    } catch {
      // Fall through to line mode below.
    }

    state.lineBuffer += chunk.toString("utf8");
    const parsedLines = tryParseLineMessages(state.lineBuffer);
    state.lineBuffer = parsedLines.rest;
    pushMessages(state, parsedLines.messages);
  });

  child.stderr.on("data", (chunk) => {
    state.stderr += chunk.toString("utf8");
    if (state.stderr.length > 4000) {
      state.stderr = state.stderr.slice(-4000);
    }
  });

  const exitPromise = new Promise((resolve) => {
    child.on("exit", (code, signal) => resolve({ code, signal }));
    child.on("error", (error) => resolve({ error }));
  });

  try {
    child.stdin.write(
      encodeMessage({
        jsonrpc: "2.0",
        id: 1,
        method: "initialize",
        params: {
          protocolVersion: PROTOCOL_VERSION,
          capabilities: {
            roots: {},
            sampling: {},
            elicitation: {},
          },
          clientInfo: { name: "goose-docs-smoke-check", version: "0.1.0" },
        },
      }),
    );

    const initialized = await Promise.race([
      waitForResponse(state, 1),
      exitPromise.then((exit) => ({ exit })),
      new Promise((resolve) => setTimeout(() => resolve({ timeout: "initialize" }), TIMEOUT_MS)),
    ]);

    if (initialized.timeout || initialized.exit || initialized.error) {
      return result(server, command, startedAt, "fail", initialized.timeout || "exited", state, initialized.exit);
    }

    if (initialized.error) {
      return result(server, command, startedAt, "fail", JSON.stringify(initialized.error), state);
    }

    child.stdin.write(
      encodeMessage({
        jsonrpc: "2.0",
        method: "notifications/initialized",
        params: {},
      }),
    );

    child.stdin.write(
      encodeMessage({
        jsonrpc: "2.0",
        id: 2,
        method: "tools/list",
        params: {},
      }),
    );

    const toolsList = await Promise.race([
      waitForResponse(state, 2),
      exitPromise.then((exit) => ({ exit })),
      new Promise((resolve) => setTimeout(() => resolve({ timeout: "tools/list" }), TIMEOUT_MS)),
    ]);

    if (toolsList.timeout || toolsList.exit || toolsList.error) {
      return result(server, command, startedAt, "fail", toolsList.timeout || "exited", state, toolsList.exit);
    }

    if (toolsList.error) {
      return result(server, command, startedAt, "fail", JSON.stringify(toolsList.error), state);
    }

    const tools = toolsList.result?.tools || [];
    return result(server, command, startedAt, "pass", `${tools.length} tools`, state, undefined, tools);
  } catch (error) {
    return result(server, command, startedAt, "fail", error.message, state);
  } finally {
    clearTimeout(timer);
    if (!child.killed) child.kill("SIGTERM");
  }
}

function result(server, command, startedAt, status, detail, state, exit, tools = []) {
  return {
    id: server.id,
    name: server.name,
    command,
    status,
    detail,
    durationMs: Date.now() - startedAt,
    exit,
    stderr: state.stderr.trim().split(/\r?\n/).slice(-8).join("\n"),
    tools: tools.map((tool) => tool.name).slice(0, 20),
  };
}

function markdown(results) {
  const now = new Date().toISOString();
  const pass = results.filter((r) => r.status === "pass").length;
  const fail = results.filter((r) => r.status === "fail").length;
  const lines = [];

  lines.push("# MCP smoke check results");
  lines.push("");
  lines.push(`Run date: ${now}`);
  lines.push("");
  lines.push(`Checked: ${results.length}`);
  lines.push(`Passed: ${pass}`);
  lines.push(`Failed: ${fail}`);
  lines.push(`Timeout per server: ${TIMEOUT_MS}ms`);
  lines.push("");
  lines.push("Scope: catalog-backed stdio servers with no required secrets and a non-empty command.");
  lines.push("");
  lines.push("| ID | Status | Detail | Duration | Tools |");
  lines.push("|---|---|---|---:|---|");

  for (const r of results) {
    lines.push(
      `| \`${escapeCell(r.id)}\` | ${r.status} | ${escapeCell(r.detail)} | ${r.durationMs}ms | ${escapeCell(r.tools.join(", "))} |`,
    );
  }

  lines.push("");
  lines.push("## Failures");
  lines.push("");

  for (const r of results.filter((item) => item.status === "fail")) {
    lines.push(`### ${r.id}`);
    lines.push("");
    lines.push(`Command: \`${r.command}\``);
    lines.push("");
    lines.push(`Detail: ${r.detail}`);
    if (r.exit) {
      lines.push("");
      lines.push(`Exit: \`${JSON.stringify(r.exit)}\``);
    }
    if (r.stderr) {
      lines.push("");
      lines.push("Recent stderr:");
      lines.push("");
      lines.push("```text");
      lines.push(r.stderr);
      lines.push("```");
    }
    lines.push("");
  }

  return lines.join("\n");
}

function escapeCell(value) {
  return String(value || "").replace(/\|/g, "\\|").replace(/\n/g, "<br>");
}

async function main() {
  const servers = JSON.parse(fs.readFileSync(SERVERS_PATH, "utf8"));
  const candidates = smokeCandidates(servers);
  const results = [];

  process.on("SIGINT", () => {
    if (results.length > 0) {
      fs.writeFileSync(OUT_PATH, `${markdown(results)}\n`);
      console.log(`\nInterrupted; wrote partial ${OUT_PATH}`);
    }
    process.exit(130);
  });

  console.log(`Checking ${candidates.length} stdio no-secret MCP servers`);

  for (const server of candidates) {
    process.stdout.write(`- ${server.id} ... `);
    const item = await checkServer(server);
    results.push(item);
    console.log(`${item.status} (${item.detail})`);
  }

  fs.writeFileSync(OUT_PATH, `${markdown(results)}\n`);
  console.log(`Wrote ${OUT_PATH}`);
  process.exit(0);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
