import type { GooseSessionNotification_unstable } from '@aaif/goose-sdk';
import type { RequestPermissionRequest, SessionNotification } from '@agentclientprotocol/sdk';
import type { Message } from '../types/message';
import {
  applyElicitationRequest as applyElicitationRequestToState,
  applyElicitationStatus as applyElicitationStatusToState,
  type ElicitationStatus,
} from './adapter/elicitations';
import { applyGooseSessionNotification } from './adapter/gooseSessionNotifications';
import { applyContentChunk, applyThoughtChunk } from './adapter/messages';
import { applyPermissionRequest as applyPermissionRequestToState } from './adapter/permissions';
import {
  type AcpChatStateChange,
  type AdapterState,
  cloneMessage,
  getGooseActiveRunId,
  getGooseConversationCursor,
  getGooseMessageCount,
  isRecord,
} from './adapter/shared';
import { applyToolCall, applyToolCallUpdate } from './adapter/tools';
import type { AcpElicitationRequest } from './elicitationRequests';

export type { AcpChatStateChange } from './adapter/shared';

export interface AcpSessionNotificationAdapter {
  apply(notification: SessionNotification): AcpChatStateChange[];
  applyGoose(notification: GooseSessionNotification_unstable): AcpChatStateChange[];
  applyPermissionRequest(request: RequestPermissionRequest): AcpChatStateChange[];
  applyElicitationRequest(request: AcpElicitationRequest): AcpChatStateChange[];
  applyElicitationStatus(elicitationId: string, status: ElicitationStatus): AcpChatStateChange[];
  getMessages(): Message[];
}

export function createAcpSessionNotificationAdapter(
  initialMessages: Message[] = [],
  localSteerTextByMessageId: Map<string, string> = new Map()
): AcpSessionNotificationAdapter {
  const state: AdapterState = {
    messages: initialMessages.map(cloneMessage),
    localSteerTextByMessageId: new Map(localSteerTextByMessageId),
  };

  return {
    apply(notification) {
      return applyAcpSessionNotification(state, notification);
    },
    applyGoose(notification) {
      return applyGooseSessionNotification(state, notification);
    },
    applyPermissionRequest(request) {
      return applyPermissionRequestToState(state, request);
    },
    applyElicitationRequest(request) {
      return applyElicitationRequestToState(state, request);
    },
    applyElicitationStatus(elicitationId, status) {
      return applyElicitationStatusToState(state, elicitationId, status);
    },
    getMessages() {
      return state.messages.map(cloneMessage);
    },
  };
}

function applyAcpSessionNotification(
  state: AdapterState,
  notification: SessionNotification
): AcpChatStateChange[] {
  const update = notification.update;

  switch (update.sessionUpdate) {
    case 'user_message_chunk':
      return applyContentChunk(state, 'user', update);
    case 'agent_message_chunk':
      return applyContentChunk(state, 'assistant', update);
    case 'agent_thought_chunk':
      return applyThoughtChunk(state, update);
    case 'tool_call':
      return applyToolCall(state, update);
    case 'tool_call_update':
      return applyToolCallUpdate(state, update);
    case 'session_info_update': {
      const activeRunId = getGooseActiveRunId(update);
      const messageCount = getGooseMessageCount(update);
      const conversationCursor = getGooseConversationCursor(update);
      if (
        !update.title &&
        activeRunId === undefined &&
        messageCount === undefined &&
        conversationCursor === undefined
      ) {
        return [];
      }

      return [
        {
          type: 'sessionInfo',
          ...(update.title ? { name: update.title } : {}),
          ...(activeRunId !== undefined ? { activeRunId } : {}),
          ...(messageCount !== undefined ? { messageCount } : {}),
          ...(conversationCursor !== undefined ? { conversationCursor } : {}),
        },
      ];
    }
    case 'usage_update':
      return [
        {
          type: 'tokenState',
          tokenState: {
            totalTokens: update.used,
            ...standardUsageAccumulatedTokenState(update),
            ...(update.cost !== undefined ? { accumulatedCost: update.cost?.amount ?? null } : {}),
          },
        },
      ];
    default:
      return [];
  }
}

function standardUsageAccumulatedTokenState(
  update: Extract<SessionNotification['update'], { sessionUpdate: 'usage_update' }>
): Extract<AcpChatStateChange, { type: 'tokenState' }>['tokenState'] {
  if (!isRecord(update._meta) || !isRecord(update._meta.goose)) {
    return {};
  }

  const { accumulatedInputTokens, accumulatedOutputTokens } = update._meta.goose;
  if (typeof accumulatedInputTokens !== 'number' || typeof accumulatedOutputTokens !== 'number') {
    return {};
  }

  return {
    accumulatedInputTokens,
    accumulatedOutputTokens,
    accumulatedTotalTokens: accumulatedInputTokens + accumulatedOutputTokens,
  };
}
