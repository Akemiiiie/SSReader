import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { delimiter, join } from 'node:path';
import { spawn, spawnSync } from 'node:child_process';

const args = process.argv.slice(2);
const env = { ...process.env };
const cargoName = process.platform === 'win32' ? 'cargo.exe' : 'cargo';
const pathKey = Object.keys(env).find(key => key.toLowerCase() === 'path') || 'PATH';

function cargoAvailable() {
  return spawnSync(cargoName, ['--version'], { env, stdio: 'ignore' }).status === 0;
}

if (!cargoAvailable()) {
  const homes = [env.CARGO_HOME, join(homedir(), '.cargo')].filter(Boolean);
  const cargoBin = homes.map(home => join(home, 'bin')).find(bin => existsSync(join(bin, cargoName)));
  if (cargoBin) {
    env[pathKey] = `${cargoBin}${delimiter}${env[pathKey] || ''}`;
  }
}

if (!cargoAvailable()) {
  console.error('未找到 cargo。请安装 Rust MSVC 工具链，然后重新运行此命令。');
  console.error('安装说明：https://v2.tauri.app/start/prerequisites/');
  process.exit(1);
}

const cli = join(process.cwd(), 'node_modules', '@tauri-apps', 'cli', 'tauri.js');
const child = spawn(process.execPath, [cli, ...args], { env, stdio: 'inherit' });
child.on('error', error => {
  console.error(`Tauri 启动失败：${error.message}`);
  process.exitCode = 1;
});
child.on('exit', code => {
  process.exitCode = code ?? 1;
});
