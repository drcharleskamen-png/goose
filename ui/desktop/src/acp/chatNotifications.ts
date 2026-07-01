import type { GooseSessionNotification_unstable } from '@aaif/goose-sdk';
import type { SessionNotification } from '@agentclientprotocol/sdk';
import { AppEvents } from '../constants/events';
import { ChatState } from '../types/chatState';
import { maybeHandlePlatformEvent } from '../utils/platform_events';
import { isRecord } from './adapter/shared';
import { toolNotificationEvent } from './adapter/toolNotifications';
import type { AcpChatSessionSnapshot } from './chatSessionStore';
import { acpChatSessionActions, acpChatSessionStore } from './chatSessionStore';

const fullReloadInvalidations = new Set(['session_info', 'config', 'extension_data']);
const invalidationReloads = new Map<string, Promise<void>>();
const invalidationReloadRequests = new Set<string>();
const deferredInvalidationReloads = new Set<string>();
const deferredInvalidationReloadSubscriptions = new Map<string, () => void>();
const conversationSyncs = new Map<string, Promise<void>>();
const conversationSyncRequests = new Set<string>();
const conversationResetRequests = new Set<string>();
const conversationSyncStartCursors = new Map<string, number>();
const conversationSyncGenerations = new Map<string, number>();
const sessionWorkGenerations = new Map<string, number>();

export function handleAcpSessionNotification(notification: SessionNotification): Promise<void> {
  const snapshotBeforeNotification = acpChatSessionStore.getSnapshot(notification.sessionId);
  const sessionNameBeforeNotification = snapshotBeforeNotification?.session?.name;
  const updatedName =
    notification.update.sessionUpdate === 'session_info_update'
      ? notification.update.title
      : undefined;
  const appliedSnapshot = acpChatSessionActions.applyAcpSessionNotification(notification) as
    | AcpChatSessionSnapshot
    | undefined;
  const snapshotAfterNotification = appliedSnapshot ?? snapshotBeforeNotification;
  maybeHandleLivePlatformEvent(notification);
  const invalidations = gooseSessionInvalidations(notification);

  if (invalidations.includes('deleted')) {
    invalidatePendingSessionWork(notification.sessionId);
    acpChatSessionActions.deleteSnapshot(notification.sessionId);
    window.dispatchEvent(
      new CustomEvent(AppEvents.SESSION_DELETED, {
        detail: { sessionId: notification.sessionId },
      })
    );
    return Promise.resolve();
  }

  if (updatedName && updatedName !== sessionNameBeforeNotification) {
    window.dispatchEvent(
      new CustomEvent(AppEvents.SESSION_RENAMED, {
        detail: { sessionId: notification.sessionId, newName: updatedName },
      })
    );
  }

  if (
    notification.update.sessionUpdate === 'session_info_update' &&
    (snapshotBeforeNotification === undefined ||
      invalidations.includes('session_info') ||
      invalidations.includes('conversation'))
  ) {
    window.dispatchEvent(new CustomEvent(AppEvents.SESSION_CREATED));
  }

  const shouldReload = shouldReloadLoadedSession(snapshotBeforeNotification, invalidations);
  const shouldReloadNow = shouldReload && canReloadSessionNow(snapshotBeforeNotification);
  if (shouldReload && !shouldReloadNow) {
    requestReloadWhenIdle(notification.sessionId, snapshotBeforeNotification);
  }

  if (invalidations.includes('conversation') && !shouldReload) {
    void syncSessionConversation(
      notification.sessionId,
      gooseConversationResetRequested(notification) ||
        snapshotBeforeNotification?.chatState === ChatState.LoadingConversation
    );
  }

  if (shouldReloadNow) {
    void reloadSessionFromInvalidation(notification.sessionId);
  } else {
    drainDeferredReloadIfIdle(notification.sessionId, snapshotAfterNotification);
  }

  return Promise.resolve();
}

function shouldReloadLoadedSession(
  snapshot: AcpChatSessionSnapshot | undefined,
  invalidations: string[]
): boolean {
  return (
    snapshot?.session !== undefined &&
    invalidations.some((scope) => fullReloadInvalidations.has(scope))
  );
}

function canReloadSessionNow(snapshot: AcpChatSessionSnapshot | undefined): boolean {
  return (
    snapshot?.chatState === ChatState.Idle &&
    snapshot.activePromptAttemptId === null &&
    snapshot.activeRunId === null &&
    snapshot.pendingCancelPromptAttemptId === null
  );
}

function drainDeferredReloadIfIdle(
  sessionId: string,
  snapshot: AcpChatSessionSnapshot | undefined
): void {
  if (!deferredInvalidationReloads.has(sessionId) || !canReloadSessionNow(snapshot)) {
    return;
  }

  clearDeferredInvalidationReload(sessionId);
  void reloadSessionFromInvalidation(sessionId);
}

function clearDeferredInvalidationReload(sessionId: string): void {
  deferredInvalidationReloads.delete(sessionId);
  const unsubscribe = deferredInvalidationReloadSubscriptions.get(sessionId);
  if (unsubscribe) {
    deferredInvalidationReloadSubscriptions.delete(sessionId);
    unsubscribe();
  }
}

function subscribeToDeferredReloadDrain(sessionId: string): void {
  if (deferredInvalidationReloadSubscriptions.has(sessionId)) {
    return;
  }

  const unsubscribe = acpChatSessionStore.subscribe(sessionId, (snapshot) => {
    drainDeferredReloadIfIdle(sessionId, snapshot);
  });
  deferredInvalidationReloadSubscriptions.set(sessionId, unsubscribe);
}

function requestReloadWhenIdle(
  sessionId: string,
  snapshot: AcpChatSessionSnapshot | undefined
): void {
  if (snapshot?.session === undefined) {
    return;
  }

  if (canReloadSessionNow(snapshot)) {
    void reloadSessionFromInvalidation(sessionId);
    return;
  }

  deferredInvalidationReloads.add(sessionId);
  subscribeToDeferredReloadDrain(sessionId);
}

function currentSessionWorkGeneration(sessionId: string): number {
  return sessionWorkGenerations.get(sessionId) ?? 0;
}

function invalidatePendingSessionWork(sessionId: string): void {
  sessionWorkGenerations.set(sessionId, currentSessionWorkGeneration(sessionId) + 1);
  clearDeferredInvalidationReload(sessionId);
  invalidationReloads.delete(sessionId);
  invalidationReloadRequests.delete(sessionId);
  invalidatePendingConversationSync(sessionId);
}

function currentConversationSyncGeneration(sessionId: string): number {
  return conversationSyncGenerations.get(sessionId) ?? 0;
}

function invalidatePendingConversationSync(sessionId: string): void {
  conversationSyncGenerations.set(sessionId, currentConversationSyncGeneration(sessionId) + 1);
  conversationSyncs.delete(sessionId);
  conversationSyncRequests.delete(sessionId);
  conversationResetRequests.delete(sessionId);
  conversationSyncStartCursors.delete(sessionId);
}

function gooseSessionInvalidations(notification: SessionNotification): string[] {
  const update = notification.update;
  if (update.sessionUpdate !== 'session_info_update' || !isRecord(update._meta)) {
    return [];
  }

  const goose = update._meta.goose;
  if (!isRecord(goose) || !Array.isArray(goose.invalidations)) {
    return [];
  }

  return goose.invalidations.filter((scope): scope is string => typeof scope === 'string');
}

function gooseConversationResetRequested(notification: SessionNotification): boolean {
  const update = notification.update;
  if (update.sessionUpdate !== 'session_info_update' || !isRecord(update._meta)) {
    return false;
  }

  const goose = update._meta.goose;
  return isRecord(goose) && goose.conversationReset === true;
}

function syncSessionConversation(sessionId: string, reset: boolean): Promise<void> {
  queueConversationSyncRequest(sessionId, reset);

  const pendingReload = invalidationReloads.get(sessionId);
  if (pendingReload) {
    return pendingReload;
  }

  const pendingSync = conversationSyncs.get(sessionId);
  if (pendingSync) {
    return pendingSync;
  }

  const sessionGeneration = currentSessionWorkGeneration(sessionId);
  const conversationGeneration = currentConversationSyncGeneration(sessionId);
  const sync = (async () => {
    for (;;) {
      if (
        sessionGeneration !== currentSessionWorkGeneration(sessionId) ||
        conversationGeneration !== currentConversationSyncGeneration(sessionId)
      ) {
        break;
      }

      conversationSyncRequests.delete(sessionId);
      const shouldReset = conversationResetRequests.delete(sessionId);
      const requestedCursor = conversationSyncStartCursors.get(sessionId);
      conversationSyncStartCursors.delete(sessionId);
      const cursor = shouldReset
        ? 0
        : (requestedCursor ?? acpChatSessionStore.getSnapshot(sessionId)?.conversationCursor ?? 0);
      const { acpFetchSessionConversation } = await import('./sessions');
      let result: Awaited<ReturnType<typeof acpFetchSessionConversation>>;
      try {
        result = await acpFetchSessionConversation(sessionId, cursor);
      } catch {
        if (
          sessionGeneration !== currentSessionWorkGeneration(sessionId) ||
          conversationGeneration !== currentConversationSyncGeneration(sessionId)
        ) {
          break;
        }

        requestReloadWhenIdle(sessionId, acpChatSessionStore.getSnapshot(sessionId));
        break;
      }
      if (
        sessionGeneration !== currentSessionWorkGeneration(sessionId) ||
        conversationGeneration !== currentConversationSyncGeneration(sessionId)
      ) {
        break;
      }
      acpChatSessionActions.applyFetchedConversation(
        sessionId,
        result.notifications,
        result.nextCursor,
        shouldReset || result.reset === true
      );

      if (!conversationSyncRequests.has(sessionId) && !conversationResetRequests.has(sessionId)) {
        break;
      }
    }
  })();

  conversationSyncs.set(sessionId, sync);
  void sync.finally(() => {
    if (conversationSyncs.get(sessionId) === sync) {
      conversationSyncs.delete(sessionId);
    }
  });
  return sync;
}

function queueConversationSyncRequest(sessionId: string, reset: boolean): void {
  conversationSyncRequests.add(sessionId);
  if (reset) {
    conversationResetRequests.add(sessionId);
    conversationSyncStartCursors.delete(sessionId);
    return;
  }

  if (conversationResetRequests.has(sessionId)) {
    return;
  }

  const cursor = acpChatSessionStore.getSnapshot(sessionId)?.conversationCursor ?? 0;
  const existingCursor = conversationSyncStartCursors.get(sessionId);
  conversationSyncStartCursors.set(
    sessionId,
    existingCursor === undefined ? cursor : Math.min(existingCursor, cursor)
  );
}

function reloadSessionFromInvalidation(sessionId: string): Promise<void> {
  const pendingReload = invalidationReloads.get(sessionId);
  if (pendingReload) {
    invalidationReloadRequests.add(sessionId);
    invalidatePendingConversationSync(sessionId);
    return pendingReload;
  }

  invalidatePendingConversationSync(sessionId);
  const generation = currentSessionWorkGeneration(sessionId);
  const reload = (async () => {
    const { acpLoadSession, sessionInfoToSession } = await import('./sessions');
    for (;;) {
      if (generation !== currentSessionWorkGeneration(sessionId)) {
        break;
      }

      invalidationReloadRequests.delete(sessionId);
      acpChatSessionActions.startSessionLoad(sessionId);
      try {
        const { sessionInfo, meta } = await acpLoadSession(sessionId);
        if (generation !== currentSessionWorkGeneration(sessionId)) {
          break;
        }
        acpChatSessionActions.finishSessionLoad(sessionId, sessionInfoToSession(sessionInfo, meta));
      } catch (error) {
        if (generation !== currentSessionWorkGeneration(sessionId)) {
          break;
        }
        acpChatSessionActions.failSessionLoad(
          sessionId,
          error instanceof Error ? error.message : String(error)
        );
      }

      if (!invalidationReloadRequests.has(sessionId)) {
        break;
      }
    }
  })();

  invalidationReloads.set(sessionId, reload);
  void reload.finally(() => {
    if (invalidationReloads.get(sessionId) === reload) {
      invalidationReloads.delete(sessionId);
    }
    if (
      generation === currentSessionWorkGeneration(sessionId) &&
      (conversationSyncRequests.has(sessionId) || conversationResetRequests.has(sessionId))
    ) {
      void syncSessionConversation(sessionId, false);
    }
  });
  return reload;
}

function maybeHandleLivePlatformEvent(notification: SessionNotification): void {
  const update = notification.update;
  if (
    update.sessionUpdate !== 'tool_call_update' ||
    update.status === 'completed' ||
    update.status === 'failed'
  ) {
    return;
  }

  const event = toolNotificationEvent(update);
  if (event?.message.method === 'platform_event') {
    maybeHandlePlatformEvent(event.message, notification.sessionId);
  }
}

export function handleAcpGooseSessionNotification(
  notification: GooseSessionNotification_unstable
): Promise<void> {
  acpChatSessionActions.applyAcpGooseSessionNotification(notification);
  return Promise.resolve();
}
