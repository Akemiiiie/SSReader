import { useState } from 'preact/hooks';
import { api, asAppError } from '../../shared/api/reader';
import { AiSettings } from './AiSettings';
import type { BackupInfo, ReadingSettings } from '../../shared/contracts/book';

export function SettingsPage({ settings, onChange, onBack, onRestored }: { settings: ReadingSettings; onChange: (value: ReadingSettings) => void; onBack: () => void; onRestored: () => void }) {
  const [selected, setSelected] = useState<BackupInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const update = (patch: Partial<ReadingSettings>) => onChange({ ...settings, ...patch });
  const themeFontColor = settings.theme === 'dark' ? '#ecebe5' : settings.theme === 'light' ? '#24282b' : '#292722';
  function captureShortcut(field: 'previousPageShortcut' | 'nextPageShortcut', event: KeyboardEvent) {
    event.preventDefault();
    if (!['Control', 'Alt', 'Shift', 'Meta'].includes(event.key)) update({ [field]: event.key });
  }

  async function backup() {
    if (!window.confirm('备份文件包含完整书籍和阅读记录，且未加密。请将其保存到安全位置。继续吗？')) return;
    setBusy(true);
    try {
      const info = await api.createBackup();
      if (info) setMessage(`备份完成：${info.bookCount} 本书，${(info.byteSize / 1024 / 1024).toFixed(1)} MiB`);
    } catch (error) { setMessage(asAppError(error).message); }
    finally { setBusy(false); }
  }

  async function inspect() {
    setBusy(true);
    try { setSelected(await api.selectBackupFile()); setMessage(null); }
    catch (error) { setSelected(null); setMessage(asAppError(error).message); }
    finally { setBusy(false); }
  }

  async function restore() {
    if (!selected || !window.confirm(`恢复此备份（${selected.bookCount} 本书）？当前本地书架将被替换。建议先备份现有数据。`)) return;
    setBusy(true);
    try { await api.restoreBackup(); setSelected(null); setMessage('恢复成功'); onRestored(); }
    catch (error) { setMessage(asAppError(error).message); }
    finally { setBusy(false); }
  }

  return <main class="page settings-page">
    <header class="page-header"><div><p class="eyebrow">PREFERENCES</p><h1>设置与备份</h1></div><button class="secondary" onClick={onBack}>返回书架</button></header>
    {message && <p class="inline-notice" role="status">{message}</p>}
    <AiSettings />
    <section class="settings-card"><h2>翻页快捷键</h2>
      <label>上一屏 <input class="shortcut-input" value={settings.previousPageShortcut} onKeyDown={event => captureShortcut('previousPageShortcut', event)} onChange={() => {}} /></label>
      <label>下一屏 <input class="shortcut-input" value={settings.nextPageShortcut} onKeyDown={event => captureShortcut('nextPageShortcut', event)} onChange={() => {}} /></label>
      <p class="muted">点击输入框后按下要使用的按键。</p>
    </section>
    <section class="settings-card"><h2>外观</h2>
      <label>阅读字体 <select value={settings.fontFamily ?? 'system'} onChange={event => update({ fontFamily: event.currentTarget.value as ReadingSettings['fontFamily'] })}><option value="system">系统字体</option><option value="book">书页衬线</option><option value="mono">等宽字体</option></select></label>
      <label>阅读字号 <input type="range" min="12" max="36" value={settings.fontSize} onChange={event => update({ fontSize: Number(event.currentTarget.value) })} /> {settings.fontSize}px</label>
      <label>应用字体颜色（设置页除外） <input class="font-color-input" type="color" value={settings.fontColor ?? themeFontColor} onChange={event => update({ fontColor: event.currentTarget.value })} /> {settings.fontColor ?? '跟随主题'}</label>
      {settings.fontColor && <button class="secondary" onClick={() => update({ fontColor: null })}>字体颜色跟随主题</button>}
      <label>行距 <input type="range" min="1.2" max="2.6" step="0.1" value={settings.lineHeight} onChange={event => update({ lineHeight: Number(event.currentTarget.value) })} /> {settings.lineHeight.toFixed(1)}</label>
      <label>背景 <select value={settings.theme} onChange={event => update({ theme: event.currentTarget.value as ReadingSettings['theme'] })}><option value="paper">纸张</option><option value="light">明亮</option><option value="dark">深色</option></select></label>
      <label>PDF 缩放 <input type="range" min="0.25" max="3" step="0.05" value={settings.pdfZoom} onChange={event => update({ pdfZoom: Number(event.currentTarget.value) })} /> {Math.round(settings.pdfZoom * 100)}%</label>
      <label>背景不透明度 <input type="range" min="0" max="1" step="0.05" value={settings.opacity} onChange={event => update({ opacity: Number(event.currentTarget.value) })} /> {Math.round(settings.opacity * 100)}%</label>
      <p class="muted">只调节背景，正文颜色与清晰度保持独立。阅读时可从标题栏进入悬浮模式。</p>
      <label><input type="checkbox" checked={settings.alwaysOnTop} onChange={event => update({ alwaysOnTop: event.currentTarget.checked })} /> 窗口置顶</label>
      <button class="secondary" onClick={() => onChange({ fontSize: 19, fontFamily: 'system', fontColor: null, lineHeight: 1.8, theme: 'paper', sidebarCollapsed: false, pdfZoom: 1, alwaysOnTop: false, opacity: 1, previousPageShortcut: 'ArrowLeft', nextPageShortcut: 'ArrowRight', windowShortcutMode: 'double' })}>恢复默认设置</button>
    </section>
    <section class="settings-card"><h2>本地备份与恢复</h2><p class="muted">备份包含受管原文件、阅读位置、书签与设置。归档未加密。</p>
      <div class="action-row"><button onClick={backup} disabled={busy}>创建备份</button><button class="secondary" onClick={inspect} disabled={busy}>选择备份并预检</button></div>
      {selected && <div class="restore-preview"><p>备份预检：{selected.bookCount} 本书 · {(selected.byteSize / 1024 / 1024).toFixed(1)} MiB · 数据库版本 {selected.schemaVersion}</p><button onClick={restore} disabled={busy}>恢复此备份</button></div>}
    </section>
    <section class="settings-card"><h2>后台运行</h2>
      <label>句点键唤出方式 <select value={settings.windowShortcutMode} onChange={event => update({ windowShortcutMode: event.currentTarget.value as ReadingSettings['windowShortcutMode'] })}><option value="hold">长按 . 显示，松开隐藏</option><option value="double">双击 . 切换显示/隐藏</option></select></label>
      <p class="muted">程序启动后默认在后台运行。长按模式需按住约 0.6 秒。句点键仍可在其他程序中正常输入；关闭主窗口后，SSReader 留在系统托盘。单击托盘图标可打开，右键选择“退出程序”可完全退出。</p>
      <p class="muted">也可使用 Ctrl+Alt+R 切换窗口显示；若组合键被占用，仍可使用句点键与系统托盘。</p>
    </section>
    <section class="settings-card"><h2>账户</h2><p>当前为本地使用，无需登录。云端账户与分享将在里程碑 3 提供。</p></section>
  </main>;
}
