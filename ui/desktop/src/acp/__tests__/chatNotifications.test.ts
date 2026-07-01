import type { SessionNotification } from '@agentclientprotocol/sdk';
import { waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AppEvents } from '../../constants/events';
import { ChatState } from '../../types/chatState';
import type { Session } from '../../types/session';
import { maybeHandlePlatformEvent } from '../../utils/platform_events';
import { handleAcpSessionNotification } from '../chatNotifications';
import type { AcpChatSessionSnapshot } from '../chatSessionStore';
import { acpChatSessionActions, acpChatSessionStore } from '../chatSessionStore';
import { acpFetchSessionConversation, acpLoadSession, sessionInfoToSession } from '../sessions';

vi.mock('../chatSessionStore', () => ({
  acpChatSessionStore: {
    getSnapshot: vi.fn(),
    subscribe: vi.fn(),
  },
  acpChatSessionActions: {
    applyAcpSessionNotification: vi.fn(),
    applyFetchedConversation: vi.fn(),
    applyAcpGooseSessionNotification: vi.fn(),
    deleteSnapshot: vi.fn(),
    startSessionLoad: vi.fn(),
    finishSessionLoad: vi.fn(),
    failSessionLoad: vi.fn(),
  },
}));

vi.mock('../../utils/platform_events', () => ({
  maybeHandlePlatformEvent: vi.fn(),
}));

vi.mock('../sessions', () => ({
  acpFetchSessionConversation: vi.fn(),
  acpLoadSession: vi.fn(),
  sessionInfoToSession: vi.fn(),
}));

const SESSION_ID = 'session-1';

function deferred<T>(): {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (error: unknown) => void;
} {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((promiseResolve, promiseReject) => {
    resolve = promiseResolve;
    reject = promiseReject;
  });
  return { promise, resolve, reject };
}

function sessionInfoUpdate(title: string): SessionNotification {
  return {
    sessionId: SESSION_ID,
    update: {
      sessionUpdate: 'session_info_update',
      title,
    },
  };
}

function activeRunUpdate(activeRunId: string | null): SessionNotification {
  return {
    sessionId: SESSION_ID,
    update: {
      sessionUpdate: 'session_info_update',
      _meta: {
        goose: {
          activeRunId,
        },
      },
    },
  };
}

function invalidationUpdate(invalidations: string[]): SessionNotification {
  return {
    sessionId: SESSION_ID,
    update: {
      sessionUpdate: 'session_info_update',
      _meta: {
        goose: {
          invalidations,
        },
      },
    },
  };
}

function platformEventToolUpdate(status: 'in_progress' | 'completed'): SessionNotification {
  return {
    sessionId: SESSION_ID,
    update: {
      sessionUpdate: 'tool_call_update',
      toolCallId: 'tool-1',
      status,
      _meta: {
        toolNotification: {
          type: 'platform_event',
          params: {
            extension: 'apps',
            event_type: 'app_created',
            app_name: 'platform-event-repro',
          },
        },
      },
    },
  };
}

function sessionWithName(name: string): Session {
  return {
    id: SESSION_ID,
    name,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    working_dir: '/tmp',
    message_count: 0,
    extension_data: {},
    source: 'test',
  } as Session;
}

function snapshotWithName(
  name: string,
  overrides: Partial<AcpChatSessionSnapshot> = {}
): AcpChatSessionSnapshot {
  return {
    session: sessionWithName(name),
    messages: [],
    tokenState: {
      inputTokens: 0,
      outputTokens: 0,
      totalTokens: 0,
      accumulatedInputTokens: 0,
      accumulatedOutputTokens: 0,
      accumulatedTotalTokens: 0,
    },
    notifications: [],
    chatState: ChatState.Idle,
    sessionLoadError: undefined,
    activePromptAttemptId: null,
    activeRunId: null,
    pendingCancelPromptAttemptId: null,
    conversationCursor: 0,
    ...overrides,
  };
}

function snapshotWithoutSession(): AcpChatSessionSnapshot {
  return {
    session: undefined,
    messages: [],
    tokenState: {
      inputTokens: 0,
      outputTokens: 0,
      totalTokens: 0,
      accumulatedInputTokens: 0,
      accumulatedOutputTokens: 0,
      accumulatedTotalTokens: 0,
    },
    notifications: [],
    chatState: ChatState.Idle,
    sessionLoadError: undefined,
    activePromptAttemptId: null,
    activeRunId: null,
    pendingCancelPromptAttemptId: null,
    conversationCursor: 0,
  };
}

describe('handleAcpSessionNotification', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(acpChatSessionStore.subscribe).mockReturnValue(() => undefined);
  });

  it('dispatches SESSION_RENAMED when a session info notification changes the name', async () => {
    const dispatchEvent = vi.spyOn(window, 'dispatchEvent');
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValueOnce(snapshotWithName('Old name'));
    vi.mocked(acpChatSessionActions.applyAcpSessionNotification).mockReturnValueOnce(
      snapshotWithName('New name')
    );

    await handleAcpSessionNotification(sessionInfoUpdate('New name'));

    expect(dispatchEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        type: AppEvents.SESSION_RENAMED,
        detail: { sessionId: SESSION_ID, newName: 'New name' },
      })
    );
  });

  it('does not dispatch SESSION_RENAMED when the name is unchanged', async () => {
    const dispatchEvent = vi.spyOn(window, 'dispatchEvent');
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValueOnce(snapshotWithName('Same name'));
    vi.mocked(acpChatSessionActions.applyAcpSessionNotification).mockReturnValueOnce(
      snapshotWithName('Same name')
    );

    await handleAcpSessionNotification(sessionInfoUpdate('Same name'));

    expect(dispatchEvent).not.toHaveBeenCalled();
  });

  it('dispatches SESSION_CREATED for unknown remote session info notifications', async () => {
    const dispatchEvent = vi.spyOn(window, 'dispatchEvent');
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValueOnce(undefined);

    await handleAcpSessionNotification(sessionInfoUpdate('Remote session'));

    expect(dispatchEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        type: AppEvents.SESSION_CREATED,
      })
    );
  });

  it('dispatches SESSION_CREATED to refresh lists for loaded session info invalidations', async () => {
    const dispatchEvent = vi.spyOn(window, 'dispatchEvent');
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValueOnce(snapshotWithName('Existing'));

    await handleAcpSessionNotification(invalidationUpdate(['session_info']));

    expect(dispatchEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        type: AppEvents.SESSION_CREATED,
      })
    );
  });

  it('dispatches SESSION_RENAMED from the notification title when the session is not loaded', async () => {
    const dispatchEvent = vi.spyOn(window, 'dispatchEvent');
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValueOnce(snapshotWithoutSession());
    vi.mocked(acpChatSessionActions.applyAcpSessionNotification).mockReturnValueOnce(
      snapshotWithoutSession()
    );

    await handleAcpSessionNotification(sessionInfoUpdate('Generated name'));

    expect(dispatchEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        type: AppEvents.SESSION_RENAMED,
        detail: { sessionId: SESSION_ID, newName: 'Generated name' },
      })
    );
  });

  it('forwards live ACP platform events to the desktop platform event handler', async () => {
    await handleAcpSessionNotification(platformEventToolUpdate('in_progress'));

    expect(maybeHandlePlatformEvent).toHaveBeenCalledWith(
      {
        method: 'platform_event',
        params: {
          extension: 'apps',
          event_type: 'app_created',
          app_name: 'platform-event-repro',
        },
      },
      SESSION_ID
    );
  });

  it('does not forward completed platform event metadata as a live desktop event', async () => {
    await handleAcpSessionNotification(platformEventToolUpdate('completed'));

    expect(maybeHandlePlatformEvent).not.toHaveBeenCalled();
  });

  it('deletes the local snapshot for deleted session invalidations', async () => {
    const dispatchEvent = vi.spyOn(window, 'dispatchEvent');

    await handleAcpSessionNotification(invalidationUpdate(['deleted']));

    expect(acpChatSessionActions.deleteSnapshot).toHaveBeenCalledWith(SESSION_ID);
    expect(dispatchEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        type: AppEvents.SESSION_DELETED,
        detail: { sessionId: SESSION_ID },
      })
    );
  });

  it('ignores pending conversation sync results after session deletion', async () => {
    const fetch = deferred<Awaited<ReturnType<typeof acpFetchSessionConversation>>>();
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValue(snapshotWithName('Existing'));
    vi.mocked(acpFetchSessionConversation).mockReturnValueOnce(fetch.promise);

    await handleAcpSessionNotification(invalidationUpdate(['conversation']));
    await waitFor(() => {
      expect(acpFetchSessionConversation).toHaveBeenCalledWith(SESSION_ID, 0);
    });

    await handleAcpSessionNotification(invalidationUpdate(['deleted']));
    fetch.resolve({
      notifications: [],
      nextCursor: 3,
      reset: false,
    });
    await Promise.resolve();

    expect(acpChatSessionActions.applyFetchedConversation).not.toHaveBeenCalled();
  });

  it('ignores pending full reload results after session deletion', async () => {
    const load = deferred<Awaited<ReturnType<typeof acpLoadSession>>>();
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValue(snapshotWithName('Existing'));
    vi.mocked(acpLoadSession).mockReturnValueOnce(load.promise);

    await handleAcpSessionNotification(invalidationUpdate(['session_info']));
    await waitFor(() => {
      expect(acpLoadSession).toHaveBeenCalledWith(SESSION_ID);
    });

    await handleAcpSessionNotification(invalidationUpdate(['deleted']));
    load.resolve({
      sessionInfo: { sessionId: SESSION_ID, cwd: '/tmp' },
      meta: {},
    } as Awaited<ReturnType<typeof acpLoadSession>>);
    await Promise.resolve();

    expect(sessionInfoToSession).not.toHaveBeenCalled();
    expect(acpChatSessionActions.finishSessionLoad).not.toHaveBeenCalled();
  });

  it('ignores pending conversation sync results after a full reload starts', async () => {
    const fetch = deferred<Awaited<ReturnType<typeof acpFetchSessionConversation>>>();
    const reloadedSession = sessionWithName('Reloaded');
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValue(snapshotWithName('Existing'));
    vi.mocked(acpFetchSessionConversation).mockReturnValueOnce(fetch.promise);
    vi.mocked(acpLoadSession).mockResolvedValueOnce({
      sessionInfo: { sessionId: SESSION_ID, cwd: '/tmp' },
      meta: {},
    } as Awaited<ReturnType<typeof acpLoadSession>>);
    vi.mocked(sessionInfoToSession).mockReturnValueOnce(reloadedSession);

    await handleAcpSessionNotification(invalidationUpdate(['conversation']));
    await waitFor(() => {
      expect(acpFetchSessionConversation).toHaveBeenCalledWith(SESSION_ID, 0);
    });

    await handleAcpSessionNotification(invalidationUpdate(['session_info']));
    await waitFor(() => {
      expect(acpChatSessionActions.finishSessionLoad).toHaveBeenCalledWith(
        SESSION_ID,
        reloadedSession
      );
    });

    fetch.resolve({
      notifications: [],
      nextCursor: 3,
      reset: false,
    });
    await Promise.resolve();

    expect(acpChatSessionActions.applyFetchedConversation).not.toHaveBeenCalled();
  });

  it('queues conversation syncs while a full reload is in progress', async () => {
    const load = deferred<Awaited<ReturnType<typeof acpLoadSession>>>();
    const reloadedSession = sessionWithName('Reloaded');
    const snapshotBeforeConversationInvalidation = snapshotWithName('Existing', {
      conversationCursor: 2,
    });
    const snapshotAfterReload = snapshotWithName('Reloaded', {
      conversationCursor: 10,
    });
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValue(
      snapshotBeforeConversationInvalidation
    );
    vi.mocked(acpLoadSession).mockReturnValueOnce(load.promise);
    vi.mocked(sessionInfoToSession).mockReturnValueOnce(reloadedSession);
    vi.mocked(acpFetchSessionConversation).mockResolvedValueOnce({
      notifications: [],
      nextCursor: 11,
      reset: false,
    });

    await handleAcpSessionNotification(invalidationUpdate(['session_info']));
    await waitFor(() => {
      expect(acpLoadSession).toHaveBeenCalledWith(SESSION_ID);
    });

    await handleAcpSessionNotification(invalidationUpdate(['conversation']));
    await Promise.resolve();
    expect(acpFetchSessionConversation).not.toHaveBeenCalled();

    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValue(snapshotAfterReload);
    load.resolve({
      sessionInfo: { sessionId: SESSION_ID, cwd: '/tmp' },
      meta: {},
    } as Awaited<ReturnType<typeof acpLoadSession>>);
    await waitFor(() => {
      expect(acpChatSessionActions.finishSessionLoad).toHaveBeenCalledWith(
        SESSION_ID,
        reloadedSession
      );
    });
    await waitFor(() => {
      expect(acpFetchSessionConversation).toHaveBeenCalledWith(SESSION_ID, 2);
      expect(acpChatSessionActions.applyFetchedConversation).toHaveBeenCalledWith(
        SESSION_ID,
        [],
        11,
        false
      );
    });
  });

  it('fetches conversation deltas for conversation invalidations', async () => {
    const dispatchEvent = vi.spyOn(window, 'dispatchEvent');
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValue(snapshotWithName('Existing'));
    vi.mocked(acpFetchSessionConversation).mockResolvedValueOnce({
      notifications: [],
      nextCursor: 3,
      reset: false,
    });

    await handleAcpSessionNotification(invalidationUpdate(['conversation']));

    await waitFor(() => {
      expect(acpFetchSessionConversation).toHaveBeenCalledWith(SESSION_ID, 0);
      expect(acpChatSessionActions.applyFetchedConversation).toHaveBeenCalledWith(
        SESSION_ID,
        [],
        3,
        false
      );
      expect(dispatchEvent).toHaveBeenCalledWith(
        expect.objectContaining({
          type: AppEvents.SESSION_CREATED,
        })
      );
    });
  });

  it('falls back to a full reload when a conversation delta fetch fails', async () => {
    const reloadedSession = sessionWithName('Reloaded');
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValue(snapshotWithName('Existing'));
    vi.mocked(acpFetchSessionConversation).mockRejectedValueOnce(new Error('fetch failed'));
    vi.mocked(acpLoadSession).mockResolvedValueOnce({
      sessionInfo: { sessionId: SESSION_ID, cwd: '/tmp' },
      meta: {},
    } as Awaited<ReturnType<typeof acpLoadSession>>);
    vi.mocked(sessionInfoToSession).mockReturnValueOnce(reloadedSession);

    await handleAcpSessionNotification(invalidationUpdate(['conversation']));

    await waitFor(() => {
      expect(acpFetchSessionConversation).toHaveBeenCalledWith(SESSION_ID, 0);
      expect(acpChatSessionActions.startSessionLoad).toHaveBeenCalledWith(SESSION_ID);
      expect(acpLoadSession).toHaveBeenCalledWith(SESSION_ID);
      expect(acpChatSessionActions.finishSessionLoad).toHaveBeenCalledWith(
        SESSION_ID,
        reloadedSession
      );
    });
    expect(acpChatSessionActions.applyFetchedConversation).not.toHaveBeenCalled();
  });

  it('resets conversation syncs when invalidations arrive during session load', async () => {
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValue(
      snapshotWithName('Existing', {
        chatState: ChatState.LoadingConversation,
      })
    );
    vi.mocked(acpFetchSessionConversation).mockResolvedValueOnce({
      notifications: [],
      nextCursor: 3,
      reset: false,
    });

    await handleAcpSessionNotification(invalidationUpdate(['conversation']));

    await waitFor(() => {
      expect(acpFetchSessionConversation).toHaveBeenCalledWith(SESSION_ID, 0);
      expect(acpChatSessionActions.applyFetchedConversation).toHaveBeenCalledWith(
        SESSION_ID,
        [],
        3,
        true
      );
    });
  });

  it('reloads the session for session info invalidations', async () => {
    const reloadedSession = sessionWithName('Reloaded');
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValueOnce(snapshotWithName('Existing'));
    vi.mocked(acpLoadSession).mockResolvedValueOnce({
      sessionInfo: { sessionId: SESSION_ID, cwd: '/tmp' },
      meta: {},
    } as Awaited<ReturnType<typeof acpLoadSession>>);
    vi.mocked(sessionInfoToSession).mockReturnValueOnce(reloadedSession);

    await handleAcpSessionNotification(invalidationUpdate(['session_info']));

    await waitFor(() => {
      expect(acpChatSessionActions.startSessionLoad).toHaveBeenCalledWith(SESSION_ID);
      expect(acpLoadSession).toHaveBeenCalledWith(SESSION_ID);
      expect(acpChatSessionActions.finishSessionLoad).toHaveBeenCalledWith(
        SESSION_ID,
        reloadedSession
      );
    });
  });

  it('defers full reloads while the session is streaming', async () => {
    const streamingSnapshot = snapshotWithName('Existing', {
      chatState: ChatState.Streaming,
      activePromptAttemptId: 'attempt-1',
      activeRunId: 'run-1',
    });
    const idleSnapshot = snapshotWithName('Existing');
    const reloadedSession = sessionWithName('Reloaded');
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValue(streamingSnapshot);
    vi.mocked(acpChatSessionActions.applyAcpSessionNotification)
      .mockReturnValueOnce(streamingSnapshot)
      .mockReturnValueOnce(idleSnapshot);
    vi.mocked(acpLoadSession).mockResolvedValueOnce({
      sessionInfo: { sessionId: SESSION_ID, cwd: '/tmp' },
      meta: {},
    } as Awaited<ReturnType<typeof acpLoadSession>>);
    vi.mocked(sessionInfoToSession).mockReturnValueOnce(reloadedSession);

    await handleAcpSessionNotification(invalidationUpdate(['config']));
    await Promise.resolve();
    expect(acpLoadSession).not.toHaveBeenCalled();
    expect(acpChatSessionActions.startSessionLoad).not.toHaveBeenCalled();

    await handleAcpSessionNotification(activeRunUpdate(null));

    await waitFor(() => {
      expect(acpLoadSession).toHaveBeenCalledWith(SESSION_ID);
      expect(acpChatSessionActions.finishSessionLoad).toHaveBeenCalledWith(
        SESSION_ID,
        reloadedSession
      );
    });
  });

  it('defers full reloads while the session is loading', async () => {
    const loadingSnapshot = snapshotWithName('Existing', {
      chatState: ChatState.LoadingConversation,
    });
    const idleSnapshot = snapshotWithName('Existing');
    const reloadedSession = sessionWithName('Reloaded');
    let loadListener: ((snapshot: AcpChatSessionSnapshot) => void) | undefined;
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValue(loadingSnapshot);
    vi.mocked(acpChatSessionActions.applyAcpSessionNotification).mockReturnValueOnce(
      loadingSnapshot
    );
    vi.mocked(acpChatSessionStore.subscribe).mockImplementationOnce((_sessionId, listener) => {
      loadListener = listener;
      return vi.fn();
    });
    vi.mocked(acpLoadSession).mockResolvedValueOnce({
      sessionInfo: { sessionId: SESSION_ID, cwd: '/tmp' },
      meta: {},
    } as Awaited<ReturnType<typeof acpLoadSession>>);
    vi.mocked(sessionInfoToSession).mockReturnValueOnce(reloadedSession);

    await handleAcpSessionNotification(invalidationUpdate(['session_info']));
    await Promise.resolve();
    expect(acpLoadSession).not.toHaveBeenCalled();
    expect(acpChatSessionActions.startSessionLoad).not.toHaveBeenCalled();

    loadListener?.(idleSnapshot);

    await waitFor(() => {
      expect(acpLoadSession).toHaveBeenCalledWith(SESSION_ID);
      expect(acpChatSessionActions.finishSessionLoad).toHaveBeenCalledWith(
        SESSION_ID,
        reloadedSession
      );
    });
  });

  it('runs a queued reload after the current reload completes', async () => {
    const firstLoad = deferred<Awaited<ReturnType<typeof acpLoadSession>>>();
    const secondLoad = deferred<Awaited<ReturnType<typeof acpLoadSession>>>();
    const firstSession = sessionWithName('First reload');
    const secondSession = sessionWithName('Second reload');
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValue(snapshotWithName('Existing'));
    vi.mocked(acpLoadSession)
      .mockReturnValueOnce(firstLoad.promise)
      .mockReturnValueOnce(secondLoad.promise);
    vi.mocked(sessionInfoToSession)
      .mockReturnValueOnce(firstSession)
      .mockReturnValueOnce(secondSession);

    await handleAcpSessionNotification(invalidationUpdate(['session_info']));

    await waitFor(() => {
      expect(acpLoadSession).toHaveBeenCalledTimes(1);
    });

    await handleAcpSessionNotification(invalidationUpdate(['config']));
    await Promise.resolve();
    expect(acpLoadSession).toHaveBeenCalledTimes(1);

    firstLoad.resolve({
      sessionInfo: { sessionId: SESSION_ID, cwd: '/tmp' },
      meta: {},
    } as Awaited<ReturnType<typeof acpLoadSession>>);

    await waitFor(() => {
      expect(acpLoadSession).toHaveBeenCalledTimes(2);
      expect(acpChatSessionActions.finishSessionLoad).toHaveBeenCalledWith(
        SESSION_ID,
        firstSession
      );
    });

    secondLoad.resolve({
      sessionInfo: { sessionId: SESSION_ID, cwd: '/tmp' },
      meta: {},
    } as Awaited<ReturnType<typeof acpLoadSession>>);

    await waitFor(() => {
      expect(acpChatSessionActions.finishSessionLoad).toHaveBeenCalledWith(
        SESSION_ID,
        secondSession
      );
    });
  });

  it('skips conversation sync when a full reload is queued', async () => {
    const reloadedSession = sessionWithName('Reloaded');
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValueOnce(snapshotWithName('Existing'));
    vi.mocked(acpLoadSession).mockResolvedValueOnce({
      sessionInfo: { sessionId: SESSION_ID, cwd: '/tmp' },
      meta: {},
    } as Awaited<ReturnType<typeof acpLoadSession>>);
    vi.mocked(sessionInfoToSession).mockReturnValueOnce(reloadedSession);

    await handleAcpSessionNotification(invalidationUpdate(['conversation', 'session_info']));

    await waitFor(() => {
      expect(acpLoadSession).toHaveBeenCalledWith(SESSION_ID);
    });
    expect(acpFetchSessionConversation).not.toHaveBeenCalled();
  });

  it('does not load a full session for list-only session info invalidations', async () => {
    vi.mocked(acpChatSessionStore.getSnapshot).mockReturnValueOnce(undefined);

    await handleAcpSessionNotification(invalidationUpdate(['session_info']));

    await Promise.resolve();
    expect(acpLoadSession).not.toHaveBeenCalled();
    expect(acpChatSessionActions.startSessionLoad).not.toHaveBeenCalled();
  });
});
