import type { GooseSessionNotification_unstable } from '@aaif/goose-sdk';
import { type AcpChatStateChange, type AdapterState, messagesChange } from './shared';

export function applyGooseSessionNotification(
  state: AdapterState,
  notification: GooseSessionNotification_unstable
): AcpChatStateChange[] {
  const update = notification.update;

  switch (update.sessionUpdate) {
    case 'usage_update': {
      // `accumulatedSavings` is sent by the server but may not yet be present
      // in the published SDK types, so read it defensively.
      const accumulatedSavings = (update as { accumulatedSavings?: number | null })
        .accumulatedSavings;
      return [
        {
          type: 'tokenState',
          tokenState: {
            totalTokens: update.used,
            accumulatedInputTokens: update.accumulatedInputTokens,
            accumulatedOutputTokens: update.accumulatedOutputTokens,
            accumulatedTotalTokens: update.accumulatedInputTokens + update.accumulatedOutputTokens,
            ...(update.accumulatedCost !== undefined
              ? { accumulatedCost: update.accumulatedCost }
              : {}),
            ...(accumulatedSavings !== undefined && accumulatedSavings !== null
              ? { accumulatedSavings }
              : {}),
          },
        },
      ];
    }
    case 'status_message':
      return applyStatusMessage(state, notification.sessionId, update);
    default:
      return [];
  }
}

function applyStatusMessage(
  state: AdapterState,
  sessionId: string,
  update: Extract<GooseSessionNotification_unstable['update'], { sessionUpdate: 'status_message' }>
): AcpChatStateChange[] {
  const notificationType = update.status.type === 'notice' ? 'inlineMessage' : 'thinkingMessage';

  state.messages.push({
    id: `acp_status_${sessionId}_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`,
    role: 'assistant',
    created: Math.floor(Date.now() / 1000),
    content: [
      {
        type: 'systemNotification',
        notificationType,
        msg: update.status.message,
      },
    ],
    metadata: {
      userVisible: true,
      agentVisible: false,
    },
  });

  return messagesChange(state);
}
