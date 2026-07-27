import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function validateAcl() {
  console.log('==> Validating Tauri command ACL');
  const main = readFileSync(path.join(rootDir, 'src-tauri/src/main.rs'), 'utf8');
  const permission = readFileSync(
    path.join(rootDir, 'src-tauri/permissions/courselib.toml'),
    'utf8',
  );
  const capability = JSON.parse(
    readFileSync(path.join(rootDir, 'src-tauri/capabilities/default.json'), 'utf8'),
  );

  const handlerBlock = main.match(/generate_handler!\[([\s\S]*?)\]/)?.[1];
  const permissionBlock = permission.match(/commands\.allow\s*=\s*\[([\s\S]*?)\]/)?.[1];
  if (!handlerBlock || !permissionBlock) {
    throw new Error('Could not read the Tauri command ACL');
  }

  const handlers = [...handlerBlock.matchAll(/commands::([a-z_]+)/g)].map((match) => match[1]);
  const allowed = [...permissionBlock.matchAll(/"([a-z_]+)"/g)].map((match) => match[1]);
  const missing = handlers.filter((command) => !allowed.includes(command));
  const stale = allowed.filter((command) => !handlers.includes(command));
  if (missing.length || stale.length) {
    throw new Error(`Tauri command ACL mismatch (missing: ${missing}; stale: ${stale})`);
  }

  if (
    !capability.remote?.urls?.includes('http://127.0.0.1:*') ||
    !capability.permissions?.includes('app-commands')
  ) {
    throw new Error('The packaged loopback origin is missing its app command ACL');
  }
}

function run(label, command, args, options = {}) {
  console.log(`==> ${label}`);
  const result = spawnSync(command, args, {
    cwd: rootDir,
    stdio: 'inherit',
    shell: process.platform === 'win32',
    ...options,
  });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

validateAcl();
run('Running Rust tests', 'cargo', ['test'], {
  cwd: path.join(rootDir, 'src-tauri'),
});
run('Building frontend', 'npm', ['run', 'build']);
run('Building Tauri debug app', 'npm', ['run', 'tauri', '--', 'build', '--debug']);
console.log('==> Validation complete');
