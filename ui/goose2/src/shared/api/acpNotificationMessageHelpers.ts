import type {
  ToolCall,
  ToolCallContent,
  ToolCallLocation,
  ToolCallUpdate,
  ToolKind,
} from "@agentclientprotocol/sdk";
import { ensureReplayBuffer } from "@/features/chat/hooks/replayBuffer";
import type { getBufferedMessage } from "@/features/chat/hooks/replayBuffer";

type ToolCallPayloadCarrier = {
  content?: ToolCall["content"] | ToolCallUpdate["content"];
  kind?: ToolCall["kind"] | ToolCallUpdate["kind"];
  locations?: ToolCall["locations"] | ToolCallUpdate["locations"];
  rawInput?: unknown;
  raw_input?: unknown;
  rawOutput?: ToolCall["rawOutput"] | ToolCallUpdate["rawOutput"];
  raw_output?: unknown;
};

export interface NormalizedToolCallPayload {
  arguments?: Record<string, unknown>;
  kind?: ToolKind;
  locations?: ToolCallLocation[];
  content?: ToolCallContent[];
  rawOutput?: unknown;
  resultText: string;
}

export function findMessageInBuffer(
  sessionId: string,
  _toolCallId: string,
): ReturnType<typeof getBufferedMessage> {
  const buffer = ensureReplayBuffer(sessionId);
  return buffer[buffer.length - 1];
}

export function findMessageWithToolCall(
  sessionId: string,
  toolCallId: string,
): ReturnType<typeof getBufferedMessage> {
  const buffer = ensureReplayBuffer(sessionId);
  for (let i = buffer.length - 1; i >= 0; i -= 1) {
    const msg = buffer[i];
    if (
      msg.content.some((content) => {
        return content.type === "toolRequest" && content.id === toolCallId;
      })
    ) {
      return msg;
    }
  }
  return buffer[buffer.length - 1];
}

export function getToolCallArguments(
  update: ToolCallPayloadCarrier,
): Record<string, unknown> | undefined {
  const rawInput = update.rawInput ?? update.raw_input;

  if (!rawInput || typeof rawInput !== "object" || Array.isArray(rawInput)) {
    return undefined;
  }

  return rawInput as Record<string, unknown>;
}

function getToolCallContent(
  update: ToolCallPayloadCarrier,
): ToolCallContent[] | undefined {
  return Array.isArray(update.content) ? update.content : undefined;
}

function getToolCallKind(update: ToolCallPayloadCarrier): ToolKind | undefined {
  return typeof update.kind === "string" ? update.kind : undefined;
}

function getToolCallLocations(
  update: ToolCallPayloadCarrier,
): ToolCallLocation[] | undefined {
  return Array.isArray(update.locations) ? update.locations : undefined;
}

function getToolCallRawOutput(update: ToolCallPayloadCarrier): unknown {
  return update.rawOutput ?? update.raw_output;
}

export function extractToolResultText(update: ToolCallPayloadCarrier): string {
  const textParts: string[] = [];

  if (update.content && update.content.length > 0) {
    for (const item of update.content) {
      if (item.type === "content" && item.content?.type === "text") {
        const text = item.content.text?.trim();
        if (text) {
          textParts.push(text);
        }
      }
    }
  }

  if (textParts.length > 0) {
    return textParts.join("\n\n");
  }

  const rawOutput = getToolCallRawOutput(update);
  if (rawOutput !== undefined && rawOutput !== null) {
    return typeof rawOutput === "string"
      ? rawOutput
      : JSON.stringify(rawOutput);
  }

  return "";
}

export function normalizeToolCallPayload(
  update: ToolCallPayloadCarrier,
): NormalizedToolCallPayload {
  return {
    arguments: getToolCallArguments(update),
    kind: getToolCallKind(update),
    locations: getToolCallLocations(update),
    content: getToolCallContent(update),
    rawOutput: getToolCallRawOutput(update),
    resultText: extractToolResultText(update),
  };
}
