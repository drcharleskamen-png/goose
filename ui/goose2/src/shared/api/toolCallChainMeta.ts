import type {
  MessageContent,
  ToolRequestContent,
} from "@/shared/types/messages";

const TOOL_CHAIN_META_KEY = "_goose/tool-chain-id";
const TOOL_CHAIN_SUMMARY_META_KEY = "_goose/tool-chain-summary";
const ACTIVE_TOOL_CHAIN_SUMMARY = "working";

export type AcpMetaCarrier = {
  _meta?: Record<string, unknown> | null;
};

export function getToolChainId(update: AcpMetaCarrier): string | undefined {
  const chainId = update._meta?.[TOOL_CHAIN_META_KEY];
  return typeof chainId === "string" ? chainId : undefined;
}

export function getToolChainSummary(
  update: AcpMetaCarrier,
): string | undefined {
  const chainSummary = update._meta?.[TOOL_CHAIN_SUMMARY_META_KEY];
  return typeof chainSummary === "string" ? chainSummary : undefined;
}

function mergeToolChainSummary(
  currentSummary: string | undefined,
  nextSummary: string | undefined,
): string | undefined {
  if (!nextSummary) {
    return currentSummary;
  }

  if (
    currentSummary &&
    currentSummary !== ACTIVE_TOOL_CHAIN_SUMMARY &&
    nextSummary === ACTIVE_TOOL_CHAIN_SUMMARY
  ) {
    return currentSummary;
  }

  return nextSummary;
}

export function updateToolCallBlocks(
  content: MessageContent[],
  toolCallId: string,
  options: {
    title?: string | null;
    chainSummary?: string;
    arguments?: Record<string, unknown>;
    kind?: ToolRequestContent["kind"];
    locations?: ToolRequestContent["locations"];
    content?: ToolRequestContent["content"];
    rawOutput?: ToolRequestContent["rawOutput"];
    markCompleted?: boolean;
  },
): MessageContent[] {
  return content.map((block) => {
    if (
      (block.type !== "toolRequest" && block.type !== "toolResponse") ||
      block.id !== toolCallId
    ) {
      return block;
    }

    const chainSummary = mergeToolChainSummary(
      block.chainSummary,
      options.chainSummary,
    );

    if (block.type === "toolRequest") {
      return {
        ...block,
        ...(options.title !== undefined && options.title !== null
          ? { name: options.title }
          : {}),
        ...(options.arguments !== undefined
          ? { arguments: options.arguments }
          : {}),
        ...(options.kind !== undefined ? { kind: options.kind } : {}),
        ...(options.locations !== undefined
          ? { locations: options.locations }
          : {}),
        ...(options.content !== undefined ? { content: options.content } : {}),
        ...(options.rawOutput !== undefined
          ? { rawOutput: options.rawOutput }
          : {}),
        ...(options.markCompleted ? { status: "completed" as const } : {}),
        ...(chainSummary !== undefined ? { chainSummary } : {}),
      };
    }

    return {
      ...block,
      ...(options.title !== undefined && options.title !== null
        ? { name: options.title }
        : {}),
      ...(options.kind !== undefined ? { kind: options.kind } : {}),
      ...(options.locations !== undefined
        ? { locations: options.locations }
        : {}),
      ...(options.content !== undefined ? { content: options.content } : {}),
      ...(options.rawOutput !== undefined
        ? { rawOutput: options.rawOutput }
        : {}),
      ...(chainSummary !== undefined ? { chainSummary } : {}),
    };
  });
}

export function findToolRequestById(
  content: MessageContent[],
  toolCallId: string,
): ToolRequestContent | null {
  for (let index = content.length - 1; index >= 0; index -= 1) {
    const block = content[index];
    if (block.type === "toolRequest" && block.id === toolCallId) {
      return block;
    }
  }

  return null;
}
