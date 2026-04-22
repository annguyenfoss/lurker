import { spawnSync } from 'node:child_process';
import { chmodSync, copyFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const appDir = path.resolve(__dirname, '..');
const repoRoot = path.resolve(appDir, '..', '..');
const profile = process.argv[2] === 'release' ? 'release' : 'debug';

const hostTriple = spawnSync('rustc', ['--print', 'host-tuple'], {
  cwd: repoRoot,
  encoding: 'utf8'
});

if (hostTriple.status !== 0) {
  process.exit(hostTriple.status ?? 1);
}

const targetTriple = hostTriple.stdout.trim();
const cargoArgs = ['build', '-p', 'lurker-helper'];
if (profile === 'release') {
  cargoArgs.push('--release');
}

const cargo = spawnSync('cargo', cargoArgs, {
  cwd: repoRoot,
  stdio: 'inherit'
});

if (cargo.status !== 0) {
  process.exit(cargo.status ?? 1);
}

const extension = process.platform === 'win32' ? '.exe' : '';
const builtBinary = path.join(repoRoot, 'target', profile, `lurker-helper${extension}`);
const destinationDir = path.join(appDir, 'src-tauri', 'binaries');
const destinationBinary = path.join(
  destinationDir,
  `lurker-helper-${targetTriple}${extension}`
);

mkdirSync(destinationDir, { recursive: true });
copyFileSync(builtBinary, destinationBinary);

if (process.platform !== 'win32') {
  chmodSync(destinationBinary, 0o755);
}
