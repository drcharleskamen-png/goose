import { acpHttpUrlFromHttpBase, statusHttpUrlFromHttpBase } from './acp/url';

export interface CheckServerStatusOptions {
  onEvent?: (name: string, details?: Record<string, unknown>) => void;
}

export interface CheckBackendStatusParams {
  baseUrl: string;
  serverSecret: string;
  fetch: typeof globalThis.fetch;
  errorLog?: string[];
  options?: CheckServerStatusOptions;
}

export const isFatalError = (line: string): boolean => {
  const fatalPatterns = [/panicked at/, /RUST_BACKTRACE/, /fatal error/i];
  return fatalPatterns.some((pattern) => pattern.test(line));
};

export const checkBackendStatus = async ({
  baseUrl,
  serverSecret,
  fetch,
  errorLog = [],
  options = {},
}: CheckBackendStatusParams): Promise<boolean> => {
  const timeout = 30000;
  const interval = 100;
  const maxAttempts = Math.ceil(timeout / interval);
  const statusUrl = statusHttpUrlFromHttpBase(baseUrl);
  const acpUrl = acpHttpUrlFromHttpBase(baseUrl, serverSecret);
  options.onEvent?.('healthcheck_start', { timeoutMs: timeout, intervalMs: interval });

  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    if (errorLog.some(isFatalError)) {
      options.onEvent?.('healthcheck_fatal_error', { attempt });
      return false;
    }

    try {
      const response = await fetch(statusUrl, {
        headers: {
          'X-Secret-Key': serverSecret,
        },
      });
      if (response.ok) {
        const authResponse = await fetch(acpUrl);
        // GET /acp without an SSE Accept header returns 406 after auth succeeds.
        if (authResponse.status === 406) {
          options.onEvent?.('healthcheck_success', { attempt });
          return true;
        }
        if (authResponse.status === 401 || authResponse.status === 403) {
          options.onEvent?.('healthcheck_auth_failed', { attempt });
          return false;
        }
      }
    } catch {
      // Retry until the backend is ready or the timeout expires.
    }

    await new Promise((resolve) => setTimeout(resolve, interval));
  }

  options.onEvent?.('healthcheck_timeout', { timeoutMs: timeout });
  return false;
};
