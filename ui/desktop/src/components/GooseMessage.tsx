import { useMemo, useRef, useState } from 'react';
import { ChartNoAxesColumnIncreasing } from 'lucide-react';
import ImagePreview from './ImagePreview';
import { formatMessageTimestamp } from '../utils/timeUtils';
import MarkdownContent from './MarkdownContent';
import ThinkingContent from './ThinkingContent';
import ToolCallWithResponse from './ToolCallWithResponse';
import {
  getTextAndImageContent,
  getThinkingContent,
  getToolRequests,
  getToolResponses,
  getToolConfirmationContent,
  getElicitationContent,
  getPendingToolConfirmationIds,
  getAnyToolConfirmationData,
  ToolConfirmationData,
  NotificationEvent,
} from '../types/message';
import { Message, ProviderUsage } from '../api';
import ToolCallConfirmation from './ToolCallConfirmation';
import ElicitationRequest from './ElicitationRequest';
import MessageCopyLink from './MessageCopyLink';
import { cn } from '../utils';
import { identifyConsecutiveToolCalls, shouldHideTimestamp } from '../utils/toolCallChaining';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from './ui/dialog';

interface GooseMessageProps {
  sessionId: string;
  message: Message;
  messages: Message[];
  metadata?: string[];
  toolCallNotifications: Map<string, NotificationEvent[]>;
  append: (value: string) => void;
  isStreaming: boolean;
  submitElicitationResponse?: (
    elicitationId: string,
    userData: Record<string, unknown>
  ) => Promise<boolean>;
}

type MessageUsageMetadata = Message['metadata'] & {
  usage?: ProviderUsage;
};

function formatCount(value: number | null | undefined): string {
  return typeof value === 'number' ? value.toLocaleString() : '—';
}

function formatDuration(valueMs: number | null | undefined): string {
  if (typeof valueMs !== 'number') return '—';
  if (valueMs < 1000) return `${valueMs} ms`;
  return `${(valueMs / 1000).toFixed(2)} s`;
}

function formatTokensPerSecond(usage: ProviderUsage): string {
  const outputTokens = usage.usage.output_tokens;
  const elapsedMs = usage.stats?.elapsed_ms;
  if (typeof outputTokens !== 'number' || typeof elapsedMs !== 'number' || elapsedMs <= 0) {
    return '—';
  }
  return `${(outputTokens / (elapsedMs / 1000)).toFixed(1)} tok/s`;
}

function UsageStatsButton({ usage }: { usage?: ProviderUsage }) {
  const [open, setOpen] = useState(false);
  if (!usage) return null;

  const rows = [
    ['Input tokens', formatCount(usage.usage.input_tokens)],
    ['Output tokens', formatCount(usage.usage.output_tokens)],
    ['Total tokens', formatCount(usage.usage.total_tokens)],
    ['Cache read', formatCount(usage.usage.cache_read_input_tokens)],
    ['Cache write', formatCount(usage.usage.cache_write_input_tokens)],
    ['Time to first token', formatDuration(usage.stats?.time_to_first_token_ms)],
    ['Total time', formatDuration(usage.stats?.elapsed_ms)],
    ['Tokens / second', formatTokensPerSecond(usage)],
    ['Model', usage.model],
  ];

  return (
    <>
      <button
        type="button"
        title="Usage stats"
        aria-label="Usage stats"
        onClick={() => setOpen(true)}
        className="inline-flex h-5 w-5 items-center justify-center rounded text-text-secondary/70 transition-colors hover:bg-background-secondary hover:text-text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <ChartNoAxesColumnIncreasing className="h-3.5 w-3.5" />
      </button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>Usage stats</DialogTitle>
          </DialogHeader>
          <div className="divide-y divide-border/40 text-sm">
            {rows.map(([label, value]) => (
              <div key={label} className="flex items-center justify-between gap-6 py-2">
                <span className="text-text-secondary">{label}</span>
                <span className="font-mono text-text-primary text-right">{value}</span>
              </div>
            ))}
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}

export default function GooseMessage({
  sessionId,
  message,
  messages,
  toolCallNotifications,
  append,
  isStreaming,
  submitElicitationResponse,
}: GooseMessageProps) {
  const contentRef = useRef<HTMLDivElement | null>(null);

  const { textContent: displayText, imagePaths } = getTextAndImageContent(message);
  const thinkingContent = getThinkingContent(message);
  const usage = (message.metadata as MessageUsageMetadata).usage;

  const timestamp = useMemo(() => formatMessageTimestamp(message.created), [message.created]);
  const toolRequests = getToolRequests(message);
  const messageIndex = messages.findIndex((msg) => msg.id === message.id);
  const toolConfirmationContent = getToolConfirmationContent(message);
  const elicitationContent = getElicitationContent(message);

  const findConfirmationForToolAcrossMessages = (
    toolRequestId: string
  ): ToolConfirmationData | undefined => {
    for (const msg of messages) {
      const confirmationData = getAnyToolConfirmationData(msg);
      if (confirmationData && confirmationData.id === toolRequestId) {
        return confirmationData;
      }
    }
    return undefined;
  };
  const toolCallChains = useMemo(() => identifyConsecutiveToolCalls(messages), [messages]);
  const hideTimestamp = useMemo(
    () => shouldHideTimestamp(messageIndex, toolCallChains),
    [messageIndex, toolCallChains]
  );
  const hasToolConfirmation = toolConfirmationContent !== undefined;
  const hasElicitation = elicitationContent !== undefined;
  const elicitationData =
    elicitationContent?.data.actionType === 'elicitation'
      ? (elicitationContent.data as typeof elicitationContent.data & {
          isSubmitted?: boolean;
          isCancelled?: boolean;
        })
      : undefined;

  const toolConfirmationShownInline = useMemo(() => {
    if (!toolConfirmationContent) return false;
    const confirmationData = getAnyToolConfirmationData(message);
    if (!confirmationData) return false;

    for (const msg of messages) {
      const requests = getToolRequests(msg);
      if (requests.some((req) => req.id === confirmationData.id)) {
        return true;
      }
    }
    return false;
  }, [toolConfirmationContent, message, messages]);

  const toolResponsesMap = useMemo(() => {
    const responseMap = new Map();

    if (messageIndex !== undefined && messageIndex >= 0) {
      for (let i = messageIndex + 1; i < messages.length; i++) {
        const responses = getToolResponses(messages[i]);

        for (const response of responses) {
          const matchingRequest = toolRequests.find((req) => req.id === response.id);
          if (matchingRequest) {
            responseMap.set(response.id, response);
          }
        }
      }
    }

    return responseMap;
  }, [messages, messageIndex, toolRequests]);

  const pendingConfirmationIds = getPendingToolConfirmationIds(messages);

  return (
    <div className="goose-message flex w-[90%] justify-start min-w-0">
      <div className="flex flex-col w-full min-w-0">
        {thinkingContent && (
          <ThinkingContent
            content={thinkingContent}
            isExpanded={
              isStreaming &&
              !displayText.trim() &&
              imagePaths.length === 0 &&
              toolRequests.length === 0
            }
          />
        )}

        {(displayText.trim() || imagePaths.length > 0) && (
          <div className="flex flex-col group">
            {displayText.trim() && (
              <div ref={contentRef} className="w-full">
                <MarkdownContent content={displayText} />
              </div>
            )}

            {imagePaths.length > 0 && (
              <div className="mt-4">
                {imagePaths.map((imagePath, index) => (
                  <ImagePreview key={index} src={imagePath} />
                ))}
              </div>
            )}

            {toolRequests.length === 0 && (
              <div className="relative flex justify-start">
                <div className="flex items-center gap-1 pt-1">
                  {!isStreaming && (
                    <div className="text-xs font-mono text-text-secondary transition-all duration-200 group-hover:-translate-y-4 group-hover:opacity-0">
                      {timestamp}
                    </div>
                  )}
                  {!isStreaming && <UsageStatsButton usage={usage} />}
                </div>
                {message.content.every((content) => content.type === 'text') && !isStreaming && (
                  <div className="absolute left-0 pt-1">
                    <MessageCopyLink text={displayText} contentRef={contentRef} />
                  </div>
                )}
              </div>
            )}
          </div>
        )}

        {toolRequests.length > 0 && (
          <div className={cn(displayText && 'mt-2')}>
            <div className="relative flex flex-col w-full">
              <div className="flex flex-col gap-3">
                {toolRequests.map((toolRequest) => {
                  const hasResponse = toolResponsesMap.has(toolRequest.id);
                  const isPending = pendingConfirmationIds.has(toolRequest.id);
                  const confirmationContent = findConfirmationForToolAcrossMessages(toolRequest.id);
                  const isApprovalClicked = confirmationContent && !isPending && hasResponse;
                  return (
                    <div className="goose-message-tool" key={toolRequest.id}>
                      <ToolCallWithResponse
                        sessionId={sessionId}
                        isCancelledMessage={false}
                        toolRequest={toolRequest}
                        toolResponse={toolResponsesMap.get(toolRequest.id)}
                        notifications={toolCallNotifications.get(toolRequest.id)}
                        isStreamingMessage={isStreaming}
                        isPendingApproval={isPending}
                        append={append}
                        confirmationContent={confirmationContent}
                        isApprovalClicked={isApprovalClicked}
                      />
                    </div>
                  );
                })}
              </div>
              <div className="flex items-center gap-1 text-xs text-text-secondary pt-1">
                {!isStreaming && !hideTimestamp && (
                  <span className="transition-all duration-200 group-hover:-translate-y-4 group-hover:opacity-0">
                    {timestamp}
                  </span>
                )}
                {!isStreaming && <UsageStatsButton usage={usage} />}
              </div>
            </div>
          </div>
        )}

        {hasToolConfirmation && !toolConfirmationShownInline && (
          <ToolCallConfirmation
            sessionId={sessionId}
            isClicked={false}
            actionRequiredContent={toolConfirmationContent}
          />
        )}

        {hasElicitation && submitElicitationResponse && (
          <ElicitationRequest
            isCancelledMessage={elicitationData?.isCancelled === true}
            isClicked={elicitationData?.isSubmitted === true}
            actionRequiredContent={elicitationContent}
            onSubmit={submitElicitationResponse}
          />
        )}
      </div>
    </div>
  );
}
