import { status } from './api';
import type { Client } from './api/client';

export interface CheckServerStatusOptions {
  onEvent?: (name: string, details?: Record<string, unknown>) => void;
}

export const isFatalError = (line: string): boolean => {
  const fatalPatterns = [/panicked at/, /RUST_BACKTRACE/, /fatal error/i];
  return fatalPatterns.some((pattern) => pattern.test(line));
};

export const checkServerStatus = async (
  client: Client,
  errorLog: string[],
  options: CheckServerStatusOptions = {}
): Promise<boolean> => {
  const timeout = 30000;
  const interval = 100;
  const maxAttempts = Math.ceil(timeout / interval);
  options.onEvent?.('healthcheck_start', { timeoutMs: timeout, intervalMs: interval });

  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    if (errorLog.some(isFatalError)) {
      options.onEvent?.('healthcheck_fatal_error', { attempt });
      return false;
    }

    try {
      await status({ client, throwOnError: true });
      options.onEvent?.('healthcheck_success', { attempt });
      return true;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, interval));
    }
  }

  options.onEvent?.('healthcheck_timeout', { timeoutMs: timeout });
  return false;
};
