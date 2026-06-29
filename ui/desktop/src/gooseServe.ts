import { spawn, type ChildProcess } from 'child_process';
import fs from 'node:fs';
import { createServer } from 'node:net';
import os from 'node:os';
import path from 'node:path';
import {
  appendTail as appendStartupTail,
  createGooseServeStartupDiagnostics,
  type GooseServeStartupDiagnostics,
} from './startupDiagnostics';

export interface Logger {
  info: (...args: unknown[]) => void;
  error: (...args: unknown[]) => void;
}

export const defaultLogger: Logger = {
  info: (...args) => console.log('[goose-serve]', ...args),
  error: (...args) => console.error('[goose-serve]', ...args),
};

export interface FindGooseBinaryOptions {
  isPackaged?: boolean;
  resourcesPath?: string;
}

export interface StartGooseServeOptions extends FindGooseBinaryOptions {
  dir?: string;
  serverSecret: string;
  env?: Record<string, string | undefined>;
  logger?: Logger;
  diagnosticsDir?: string;
}

export interface GooseServeResult {
  acpUrl: string;
  workingDir: string;
  process: ChildProcess;
  errorLog: string[];
  cleanup: () => Promise<void>;
  startupDiagnosticsPath: string | null;
  getStartupDiagnostics: () => GooseServeStartupDiagnostics | null;
  recordStartupEvent: (name: string, details?: Record<string, unknown>) => void;
}

const existingFile = (candidate: string): boolean => {
  try {
    return fs.existsSync(candidate) && fs.statSync(candidate).isFile();
  } catch {
    return false;
  }
};

export const findGooseBinaryPath = (options: FindGooseBinaryOptions = {}): string => {
  const { isPackaged = false, resourcesPath } = options;
  const pathFromEnv = process.env.GOOSE_BINARY;
  if (pathFromEnv) {
    if (isPackaged) {
      throw new Error('GOOSE_BINARY is only supported in development builds');
    }

    const resolvedPath = path.resolve(pathFromEnv);
    if (existingFile(resolvedPath)) {
      return resolvedPath;
    }
    throw new Error(`Invalid GOOSE_BINARY path: ${pathFromEnv} (pwd is ${process.cwd()})`);
  }

  const binaryName = process.platform === 'win32' ? 'goose.exe' : 'goose';
  const possiblePaths: string[] = [];

  if (isPackaged && resourcesPath) {
    possiblePaths.push(path.join(resourcesPath, 'bin', binaryName));
    possiblePaths.push(path.join(resourcesPath, binaryName));
  } else {
    possiblePaths.push(
      path.join(process.cwd(), '..', '..', 'target', 'debug', binaryName),
      path.join(process.cwd(), '..', '..', 'target', 'release', binaryName),
      path.join(process.cwd(), 'src', 'bin', binaryName)
    );
  }

  for (const candidate of possiblePaths) {
    if (existingFile(candidate)) {
      return candidate;
    }
  }

  throw new Error(
    `Goose binary not found in any of the possible paths: ${possiblePaths.join(', ')}`
  );
};

const findAvailablePort = (): Promise<number> => {
  return new Promise((resolve, reject) => {
    const server = createServer();

    server.on('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address() as { port: number };
      server.close(() => {
        resolve(port);
      });
    });
  });
};

const delay = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

const isFatalError = (line: string): boolean => {
  const fatalPatterns = [/panicked at/, /RUST_BACKTRACE/, /fatal error/i];
  return fatalPatterns.some((pattern) => pattern.test(line));
};

const appendErrorTail = (target: string[], lines: string[], maxLines = 100): void => {
  for (const line of lines) {
    if (line.trim()) {
      target.push(line);
    }
  }
  if (target.length > maxLines) {
    target.splice(0, target.length - maxLines);
  }
};

const fetchStatus = async (statusUrl: string): Promise<boolean> => {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 1000);

  try {
    const response = await fetch(statusUrl, { signal: controller.signal });
    return response.ok;
  } catch {
    return false;
  } finally {
    clearTimeout(timeout);
  }
};

const waitForGooseServeReady = async (
  statusUrl: string,
  errorLog: string[],
  shouldStopWaiting: () => boolean,
  options: {
    healthUrl: string;
    onEvent?: (name: string, details?: Record<string, unknown>) => void;
  }
): Promise<boolean> => {
  const timeout = 30000;
  const interval = 100;
  const deadline = Date.now() + timeout;
  const probeDetails = {
    transport: 'plain-http',
    method: 'GET',
    path: '/status',
    url: statusUrl,
    statusUrl,
    healthUrl: options.healthUrl,
  };
  options.onEvent?.('healthcheck_start', {
    ...probeDetails,
    timeoutMs: timeout,
    intervalMs: interval,
  });

  let attempt = 1;
  while (Date.now() < deadline) {
    if (shouldStopWaiting()) {
      options.onEvent?.('healthcheck_fatal_error', {
        ...probeDetails,
        attempt,
        reason: 'process_unavailable',
      });
      return false;
    }

    if (errorLog.some(isFatalError)) {
      options.onEvent?.('healthcheck_fatal_error', {
        ...probeDetails,
        attempt,
        reason: 'fatal_stderr',
      });
      return false;
    }

    if (await fetchStatus(statusUrl)) {
      options.onEvent?.('healthcheck_success', {
        ...probeDetails,
        attempt,
      });
      return true;
    }

    await delay(interval);
    attempt += 1;
  }

  options.onEvent?.('healthcheck_timeout', { ...probeDetails, timeoutMs: timeout });
  return false;
};

const buildAcpUrl = (port: number, token: string): string => {
  const url = new URL(`http://127.0.0.1:${port}/acp`);
  url.protocol = 'ws:';
  url.searchParams.set('token', token);
  return url.toString();
};

const buildRedactedAcpUrl = (port: number): string => {
  const url = new URL(`http://127.0.0.1:${port}/acp`);
  url.protocol = 'ws:';
  url.searchParams.set('token', 'REDACTED');
  return url.toString();
};

const errorMessage = (error: unknown): string => {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
};

const withStartupDiagnosticsPath = (
  message: string,
  startupDiagnosticsPath: string | null
): string => {
  if (!startupDiagnosticsPath) {
    return message;
  }
  return `${message} Startup diagnostics: ${startupDiagnosticsPath}`;
};

const buildGooseServeEnv = (
  serverSecret: string,
  binaryPath: string,
  additionalEnv: Record<string, string | undefined>
): Record<string, string | undefined> => {
  const homeDir = process.env.HOME || os.homedir();
  const pathKey = process.platform === 'win32' ? 'Path' : 'PATH';
  const currentPath = process.env[pathKey] || '';

  const env: Record<string, string | undefined> = {
    ...process.env,
    HOME: homeDir,
    [pathKey]: `${path.dirname(binaryPath)}${path.delimiter}${currentPath}`,
  };

  if (process.platform === 'win32') {
    env.USERPROFILE = homeDir;
    env.APPDATA = process.env.APPDATA || path.join(homeDir, 'AppData', 'Roaming');
    env.LOCALAPPDATA = process.env.LOCALAPPDATA || path.join(homeDir, 'AppData', 'Local');
  }

  for (const [key, value] of Object.entries(additionalEnv)) {
    if (value !== undefined) {
      env[key] = value;
    }
  }

  env.GOOSE_SERVER__SECRET_KEY = serverSecret;

  return env;
};

export const startGooseServe = async ({
  dir,
  serverSecret,
  env: additionalEnv = {},
  isPackaged,
  resourcesPath,
  logger = defaultLogger,
  diagnosticsDir,
}: StartGooseServeOptions): Promise<GooseServeResult> => {
  const workingDir = dir || process.cwd();
  const startupTrace = createGooseServeStartupDiagnostics(diagnosticsDir, workingDir);
  const startupDiagnosticsPath = startupTrace?.diagnosticsPath ?? null;
  const secretKey = serverSecret.trim();
  if (!secretKey) {
    const message = 'GOOSE_SERVER__SECRET_KEY is required for goose serve';
    startupTrace?.record('configuration_error', { message });
    throw new Error(withStartupDiagnosticsPath(message, startupDiagnosticsPath));
  }

  let goosePath: string;
  try {
    goosePath = findGooseBinaryPath({ isPackaged, resourcesPath });
  } catch (error) {
    const message = errorMessage(error);
    startupTrace?.record('binary_resolve_error', { message });
    throw new Error(withStartupDiagnosticsPath(message, startupDiagnosticsPath));
  }

  const port = await findAvailablePort();
  const httpBaseUrl = `http://127.0.0.1:${port}`;
  const statusUrl = `${httpBaseUrl}/status`;
  const healthUrl = `${httpBaseUrl}/health`;
  const acpUrl = buildAcpUrl(port, secretKey);
  const redactedAcpUrl = buildRedactedAcpUrl(port);
  const errorLog: string[] = [];
  const args = ['serve', '--platform', 'desktop', '--host', '127.0.0.1', '--port', String(port)];

  logger.info(`Starting goose serve from: ${goosePath} on port ${port} in dir ${workingDir}`);
  if (startupTrace) {
    startupTrace.diagnostics.binaryPath = goosePath;
    startupTrace.diagnostics.httpBaseUrl = httpBaseUrl;
    startupTrace.diagnostics.readinessUrl = statusUrl;
    startupTrace.diagnostics.statusUrl = statusUrl;
    startupTrace.diagnostics.healthUrl = healthUrl;
    startupTrace.diagnostics.acpUrl = redactedAcpUrl;
    startupTrace.record('spawn_start', {
      binaryPath: goosePath,
      port,
      workingDir,
      args,
    });
  }

  const gooseProcess = spawn(goosePath, args, {
    env: buildGooseServeEnv(secretKey, goosePath, additionalEnv),
    cwd: workingDir,
    windowsHide: true,
    detached: process.platform === 'win32',
    shell: false,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  if (startupTrace) {
    startupTrace.diagnostics.pid = gooseProcess.pid ?? null;
    startupTrace.record('spawn_success', { pid: gooseProcess.pid ?? null });
  }

  let exited = false;
  let spawnFailed = false;
  let exitCode: number | null = null;
  let exitSignal: string | null = null;

  gooseProcess.stdout?.resume();

  const onStderrData = (data: Buffer) => {
    const lines = data.toString().split('\n');
    appendErrorTail(errorLog, lines);
    if (startupTrace) {
      appendStartupTail(startupTrace.diagnostics.stderrTail, lines);
    }
    for (const line of lines) {
      if (line.trim() && isFatalError(line)) {
        logger.error(`goose serve stderr for port ${port} and dir ${workingDir}: ${line}`);
      }
    }
  };

  gooseProcess.stderr?.on('data', onStderrData);

  gooseProcess.on('exit', (code, signal) => {
    exited = true;
    exitCode = code;
    exitSignal = signal;
    logger.info(
      `goose serve process exited with code ${code} and signal ${signal} for port ${port} and dir ${workingDir}`
    );
    if (startupTrace) {
      startupTrace.diagnostics.childExitCode = code;
      startupTrace.diagnostics.childExitSignal = signal;
      startupTrace.record('child_exit', { code, signal });
    }
  });

  gooseProcess.on('error', (error) => {
    spawnFailed = true;
    errorLog.push(error.message);
    logger.error(`Failed to start goose serve on port ${port} and dir ${workingDir}`, error);
    startupTrace?.record('spawn_error', { message: error.message, name: error.name });
  });

  const cleanup = async (): Promise<void> => {
    return new Promise<void>((resolve) => {
      if (exited || gooseProcess.killed) {
        resolve();
        return;
      }

      let resolved = false;
      const finish = () => {
        if (!resolved) {
          resolved = true;
          resolve();
        }
      };

      gooseProcess.once('close', finish);

      logger.info('Terminating goose serve');
      try {
        if (process.platform === 'win32') {
          if (gooseProcess.pid) {
            spawn('taskkill', ['/pid', gooseProcess.pid.toString(), '/f', '/t']);
          }
        } else {
          gooseProcess.kill('SIGTERM');
        }
      } catch (error) {
        logger.error('Error while terminating goose serve process:', error);
      }

      setTimeout(() => {
        if (!exited && !gooseProcess.killed && process.platform !== 'win32') {
          gooseProcess.kill('SIGKILL');
        }
        finish();
      }, 5000);
    });
  };

  const ready = await waitForGooseServeReady(statusUrl, errorLog, () => exited || spawnFailed, {
    healthUrl,
    onEvent: startupTrace?.record,
  });
  gooseProcess.stderr?.off('data', onStderrData);
  gooseProcess.stderr?.resume();

  if (!ready) {
    await cleanup();
    const exitDetails = exited
      ? ` Process exited with code ${exitCode} and signal ${exitSignal}.`
      : '';
    const stderrDetails = errorLog.length ? ` Stderr: ${errorLog.join('\n')}` : '';
    throw new Error(
      withStartupDiagnosticsPath(
        `goose serve did not become ready on ${statusUrl}.${exitDetails}${stderrDetails}`,
        startupDiagnosticsPath
      )
    );
  }

  return {
    acpUrl,
    workingDir,
    process: gooseProcess,
    errorLog,
    cleanup,
    startupDiagnosticsPath,
    getStartupDiagnostics: () => startupTrace?.diagnostics ?? null,
    recordStartupEvent: (name, details) => startupTrace?.record(name, details),
  };
};
