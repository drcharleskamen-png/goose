import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { findGooseBinaryPath } from './gooseServe';

const binaryName = process.platform === 'win32' ? 'goose.exe' : 'goose';
const tempDirs: string[] = [];
const originalCwd = process.cwd();

function makeTempDir(): string {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'goose-serve-test-'));
  tempDirs.push(tempDir);
  return tempDir;
}

function makeFile(filePath: string): string {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, '');
  fs.chmodSync(filePath, 0o755);
  return filePath;
}

describe('findGooseBinaryPath', () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    process.chdir(originalCwd);

    while (tempDirs.length > 0) {
      const tempDir = tempDirs.pop();
      if (tempDir) {
        fs.rmSync(tempDir, { recursive: true, force: true });
      }
    }
  });

  it('uses GOOSE_BINARY in development builds', () => {
    const tempDir = makeTempDir();
    const overridePath = makeFile(path.join(tempDir, 'override-goose'));
    vi.stubEnv('GOOSE_BINARY', overridePath);

    expect(findGooseBinaryPath({ isPackaged: false })).toBe(overridePath);
  });

  it('rejects GOOSE_BINARY in packaged builds', () => {
    const tempDir = makeTempDir();
    const resourcesPath = path.join(tempDir, 'resources');
    const overridePath = makeFile(path.join(tempDir, 'override-goose'));
    makeFile(path.join(resourcesPath, 'bin', binaryName));
    vi.stubEnv('GOOSE_BINARY', overridePath);

    expect(() => findGooseBinaryPath({ isPackaged: true, resourcesPath })).toThrow(
      'GOOSE_BINARY is only supported in development builds'
    );
  });

  it('prefers the debug target over the staged binary in development builds', () => {
    const tempDir = makeTempDir();
    const desktopDir = path.join(tempDir, 'ui', 'desktop');
    const stagedPath = makeFile(path.join(desktopDir, 'src', 'bin', binaryName));
    const debugPath = makeFile(path.join(tempDir, 'target', 'debug', binaryName));
    makeFile(path.join(tempDir, 'target', 'release', binaryName));
    process.chdir(desktopDir);

    const resolvedPath = findGooseBinaryPath({ isPackaged: false });
    expect(fs.realpathSync(resolvedPath)).toBe(fs.realpathSync(debugPath));
    expect(fs.realpathSync(resolvedPath)).not.toBe(fs.realpathSync(stagedPath));
  });

  it('uses the bundled goose binary in packaged builds', () => {
    const tempDir = makeTempDir();
    const resourcesPath = path.join(tempDir, 'resources');
    const bundledPath = makeFile(path.join(resourcesPath, 'bin', binaryName));

    expect(findGooseBinaryPath({ isPackaged: true, resourcesPath })).toBe(bundledPath);
  });
});
