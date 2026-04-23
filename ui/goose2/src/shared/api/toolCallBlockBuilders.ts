import type { ToolCall, ToolCallUpdate } from "@agentclientprotocol/sdk";
import type {
  ToolRequestContent,
  ToolResponseContent,
} from "@/shared/types/messages";
import {
  getToolChainId,
  getToolChainSummary,
  type AcpMetaCarrier,
} from "./toolCallChainMeta";
import type { NormalizedToolCallPayload } from "./acpNotificationMessageHelpers";

type ToolCallWithMeta = ToolCall & AcpMetaCarrier;
type ToolCallUpdateWithMeta = ToolCallUpdate & AcpMetaCarrier;

export interface ToolCallBlockPatch {
  title?: string | null;
  chainSummary?: string;
  arguments?: Record<string, unknown>;
  kind?: ToolRequestContent["kind"];
  locations?: ToolRequestContent["locations"];
  content?: ToolRequestContent["content"];
  rawOutput?: ToolRequestContent["rawOutput"];
  markCompleted?: boolean;
}

export function buildToolRequestBlock(
  update: ToolCallWithMeta,
  payload: NormalizedToolCallPayload,
): ToolRequestContent {
  return {
    type: "toolRequest",
    id: update.toolCallId,
    chainId: getToolChainId(update),
    chainSummary: getToolChainSummary(update),
    name: update.title,
    arguments: payload.arguments ?? {},
    kind: payload.kind,
    locations: payload.locations,
    content: payload.content,
    rawOutput: payload.rawOutput,
    status: "executing",
    startedAt: Date.now(),
  };
}

export function hasToolCallPatch(
  update: ToolCallUpdateWithMeta,
  payload: NormalizedToolCallPayload,
): boolean {
  return Boolean(
    update.title ||
      getToolChainSummary(update) ||
      payload.arguments ||
      payload.kind ||
      payload.locations ||
      payload.content ||
      payload.rawOutput !== undefined,
  );
}

export function buildToolCallPatch(
  update: ToolCallUpdateWithMeta,
  payload: NormalizedToolCallPayload,
  markCompleted = false,
): ToolCallBlockPatch {
  return {
    title: update.title,
    chainSummary: getToolChainSummary(update),
    arguments: payload.arguments,
    kind: payload.kind,
    locations: payload.locations,
    content: payload.content,
    rawOutput: payload.rawOutput,
    markCompleted,
  };
}

export function buildToolResponseBlock(
  update: ToolCallUpdateWithMeta,
  payload: NormalizedToolCallPayload,
  toolRequest: ToolRequestContent | null,
): ToolResponseContent {
  return {
    type: "toolResponse",
    id: update.toolCallId,
    chainId: getToolChainId(update) ?? toolRequest?.chainId,
    chainSummary: getToolChainSummary(update) ?? toolRequest?.chainSummary,
    name: toolRequest?.name ?? "",
    result: payload.resultText,
    kind: payload.kind ?? toolRequest?.kind,
    locations: payload.locations ?? toolRequest?.locations,
    content: payload.content ?? toolRequest?.content,
    rawOutput: payload.rawOutput ?? toolRequest?.rawOutput,
    isError: update.status === "failed",
  };
}
