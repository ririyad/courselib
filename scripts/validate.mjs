import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

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

run('Running Rust tests', 'cargo', ['test'], {
  cwd: path.join(rootDir, 'src-tauri'),
});
run('Building frontend', 'npm', ['run', 'build']);
run('Building Tauri debug app', 'npm', ['run', 'tauri', '--', 'build', '--debug']);
console.log('==> Validation complete');
