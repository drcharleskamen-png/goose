import { beforeEach, describe, expect, it } from "vitest";
import type { SessionNotification } from "@agentclientprotocol/sdk";
import { useChatStore } from "@/features/chat/stores/chatStore";
import {
  clearReplayBuffer,
  getAndDeleteReplayBuffer,
} from "@/features/chat/hooks/replayBuffer";
import { registerSession } from "./acpSessionTracker";
import { handleSessionNotification } from "./acpNotificationHandler";

describe("acpNotificationHandler", () => {
  beforeEach(() => {
    clearReplayBuffer("draft-session-1");
    clearReplayBuffer("draft-session-2");
    clearReplayBuffer("draft-session-3");
    clearReplayBuffer("draft-session-4");
    useChatStore.setState({
      messagesBySession: {},
      sessionStateById: {},
      queuedMessageBySession: {},
      draftsBySession: {},
      activeSessionId: null,
      isConnected: false,
      loadingSessionIds: new Set<string>(),
      scrollTargetMessageBySession: {},
    });
  });

  it("buffers usage updates until the local session mapping is registered", async () => {
    const notification = {
      sessionId: "goose-session-1",
      update: {
        sessionUpdate: "usage_update",
        used: 512,
        size: 8192,
      },
    } as SessionNotification;

    await handleSessionNotification(notification);

    expect(
      useChatStore.getState().sessionStateById["draft-session-1"],
    ).toBeUndefined();
    expect(
      useChatStore.getState().sessionStateById["goose-session-1"],
    ).toBeUndefined();

    registerSession("draft-session-1", "goose-session-1", "goose", "/tmp");

    const runtime = useChatStore
      .getState()
      .getSessionRuntime("draft-session-1");
    expect(runtime.tokenState.accumulatedTotal).toBe(512);
    expect(runtime.tokenState.contextLimit).toBe(8192);
    expect(runtime.hasUsageSnapshot).toBe(true);
  });

  it("does not buffer non-usage updates before the local session mapping exists", async () => {
    const notification = {
      sessionId: "goose-session-2",
      update: {
        sessionUpdate: "agent_message_chunk",
        messageId: "message-1",
        content: {
          type: "text",
          text: "hello from replay",
        },
      },
    } as SessionNotification;

    await handleSessionNotification(notification);
    registerSession("draft-session-2", "goose-session-2", "goose", "/tmp");

    expect(getAndDeleteReplayBuffer("draft-session-2")).toBeUndefined();
    expect(
      useChatStore.getState().messagesBySession["draft-session-2"],
    ).toBeUndefined();
  });

  it("stores live tool chain ids and falls back to the request chain on completion", async () => {
    registerSession("draft-session-3", "goose-session-3", "goose", "/tmp");

    await handleSessionNotification({
      sessionId: "goose-session-3",
      update: {
        sessionUpdate: "agent_message_chunk",
        messageId: "message-1",
        content: {
          type: "text",
          text: "Working on it",
        },
      },
    } as SessionNotification);

    await handleSessionNotification({
      sessionId: "goose-session-3",
      update: {
        sessionUpdate: "tool_call",
        toolCallId: "tool-1",
        title: "readFile",
        kind: "execute",
        rawInput: {
          command: "tree -L 2",
        },
        _meta: {
          "_goose/tool-chain-id": "chain-1",
          "_goose/tool-chain-summary": "working",
        },
      },
    } as SessionNotification);

    await handleSessionNotification({
      sessionId: "goose-session-3",
      update: {
        sessionUpdate: "tool_call_update",
        toolCallId: "tool-1",
        status: "completed",
        kind: "execute",
        locations: [{ path: "/tmp/project", line: 1 }],
        rawOutput: {
          exitCode: 0,
        },
        _meta: {
          "_goose/tool-chain-id": "chain-1",
          "_goose/tool-chain-summary": "working",
        },
        content: [
          {
            type: "content",
            content: {
              type: "text",
              text: "done",
            },
          },
        ],
      },
    } as SessionNotification);

    await handleSessionNotification({
      sessionId: "goose-session-3",
      update: {
        sessionUpdate: "tool_call_update",
        toolCallId: "tool-1",
        _meta: {
          "_goose/tool-chain-id": "chain-1",
          "_goose/tool-chain-summary": "reviewing files",
        },
      },
    } as SessionNotification);

    await handleSessionNotification({
      sessionId: "goose-session-3",
      update: {
        sessionUpdate: "tool_call_update",
        toolCallId: "tool-1",
        title: "read project files",
        _meta: {
          "_goose/tool-chain-id": "chain-1",
          "_goose/tool-chain-summary": "working",
        },
      },
    } as SessionNotification);

    const message =
      useChatStore.getState().messagesBySession["draft-session-3"]?.[0];
    expect(message).toBeDefined();
    expect(message?.content).toEqual([
      {
        type: "text",
        text: "Working on it",
      },
      {
        type: "toolRequest",
        id: "tool-1",
        chainId: "chain-1",
        chainSummary: "reviewing files",
        name: "read project files",
        arguments: {
          command: "tree -L 2",
        },
        kind: "execute",
        locations: [{ path: "/tmp/project", line: 1 }],
        content: [
          {
            type: "content",
            content: {
              type: "text",
              text: "done",
            },
          },
        ],
        rawOutput: {
          exitCode: 0,
        },
        status: "completed",
        startedAt: expect.any(Number),
      },
      {
        type: "toolResponse",
        id: "tool-1",
        chainId: "chain-1",
        chainSummary: "reviewing files",
        name: "read project files",
        result: "done",
        kind: "execute",
        locations: [{ path: "/tmp/project", line: 1 }],
        content: [
          {
            type: "content",
            content: {
              type: "text",
              text: "done",
            },
          },
        ],
        rawOutput: {
          exitCode: 0,
        },
        isError: false,
      },
    ]);
  });

  it("stores replay tool chain ids from ACP metadata", async () => {
    registerSession("draft-session-4", "goose-session-4", "goose", "/tmp");
    useChatStore.setState({
      loadingSessionIds: new Set(["draft-session-4"]),
    });

    await handleSessionNotification({
      sessionId: "goose-session-4",
      update: {
        sessionUpdate: "agent_message_chunk",
        messageId: "message-1",
        content: {
          type: "text",
          text: "Working on it",
        },
      },
    } as SessionNotification);

    await handleSessionNotification({
      sessionId: "goose-session-4",
      update: {
        sessionUpdate: "tool_call",
        toolCallId: "tool-1",
        title: "readFile",
        kind: "read",
        rawInput: {
          path: "/tmp/project/main.js",
        },
        _meta: {
          "_goose/tool-chain-id": "chain-1",
          "_goose/tool-chain-summary": "reviewing files",
        },
      },
    } as SessionNotification);

    await handleSessionNotification({
      sessionId: "goose-session-4",
      update: {
        sessionUpdate: "tool_call_update",
        toolCallId: "tool-1",
        status: "completed",
        locations: [{ path: "/tmp/project/main.js", line: 12 }],
        _meta: {
          "_goose/tool-chain-id": "chain-1",
          "_goose/tool-chain-summary": "reviewing files",
        },
        content: [
          {
            type: "content",
            content: {
              type: "text",
              text: "done",
            },
          },
        ],
      },
    } as SessionNotification);

    const buffer = getAndDeleteReplayBuffer("draft-session-4");
    expect(buffer?.[0]?.content).toEqual([
      {
        type: "text",
        text: "Working on it",
      },
      {
        type: "toolRequest",
        id: "tool-1",
        chainId: "chain-1",
        chainSummary: "reviewing files",
        name: "readFile",
        arguments: {
          path: "/tmp/project/main.js",
        },
        kind: "read",
        locations: [{ path: "/tmp/project/main.js", line: 12 }],
        content: [
          {
            type: "content",
            content: {
              type: "text",
              text: "done",
            },
          },
        ],
        status: "completed",
        startedAt: expect.any(Number),
      },
      {
        type: "toolResponse",
        id: "tool-1",
        chainId: "chain-1",
        chainSummary: "reviewing files",
        name: "readFile",
        result: "done",
        kind: "read",
        locations: [{ path: "/tmp/project/main.js", line: 12 }],
        content: [
          {
            type: "content",
            content: {
              type: "text",
              text: "done",
            },
          },
        ],
        isError: false,
      },
    ]);
  });
});
