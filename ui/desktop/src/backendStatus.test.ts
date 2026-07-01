import { describe, expect, it, vi } from 'vitest';
import { checkBackendStatus } from './backendStatus';

type FetchInit = Parameters<typeof globalThis.fetch>[1];

describe('checkBackendStatus', () => {
  it('checks /status and validates the secret against /acp', async () => {
    const fetch = vi.fn(async (input: string, init?: FetchInit) => {
      if (input === 'https://example.com/goose/status') {
        expect(init?.headers).toEqual({ 'X-Secret-Key': 'test-secret' });
        return new Response(null, { status: 200 });
      }
      if (input === 'https://example.com/goose/acp?token=test-secret') {
        expect(init).toBeUndefined();
        return new Response(null, { status: 406 });
      }

      throw new Error(`Unexpected URL: ${input}`);
    });

    await expect(
      checkBackendStatus({
        baseUrl: 'https://example.com/goose',
        serverSecret: 'test-secret',
        fetch,
      })
    ).resolves.toBe(true);

    expect(fetch).toHaveBeenCalledTimes(2);
    expect(fetch.mock.calls.map(([input]) => input)).toEqual([
      'https://example.com/goose/status',
      'https://example.com/goose/acp?token=test-secret',
    ]);
  });

  it('fails immediately when the ACP auth probe rejects the secret', async () => {
    const onEvent = vi.fn();
    const fetch = vi.fn(async (input: string) => {
      if (input === 'https://example.com/status') {
        return new Response(null, { status: 200 });
      }
      if (input === 'https://example.com/acp?token=wrong-secret') {
        return new Response(null, { status: 401 });
      }

      throw new Error(`Unexpected URL: ${input}`);
    });

    await expect(
      checkBackendStatus({
        baseUrl: 'https://example.com',
        serverSecret: 'wrong-secret',
        fetch,
        options: { onEvent },
      })
    ).resolves.toBe(false);

    expect(fetch).toHaveBeenCalledTimes(2);
    expect(onEvent).toHaveBeenCalledWith('healthcheck_auth_failed', { attempt: 1 });
  });
});
