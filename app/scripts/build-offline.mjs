import { copyFileSync, mkdirSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

const result = spawnSync(process.execPath, [join('scripts', 'tauri.mjs'), 'build', '--config', join('src-tauri', 'tauri.offline.conf.json')], { stdio: 'inherit' });
if (result.status !== 0) process.exit(result.status ?? 1);

const bundle = join('src-tauri', 'target', 'release', 'bundle');
const nsis = join(bundle, 'nsis');
const installer = readdirSync(nsis).find(name => name.endsWith('-setup.exe'));
if (!installer) throw new Error('离线 NSIS 安装包未生成');
const destination = join(bundle, 'offline');
mkdirSync(destination, { recursive: true });
const name = installer.replace(/-setup\.exe$/, '-offline-setup.exe');
copyFileSync(join(nsis, installer), join(destination, name));
console.log(`离线安装包：${join(destination, name)}`);
