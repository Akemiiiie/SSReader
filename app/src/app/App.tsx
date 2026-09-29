import { useEffect, useRef, useState } from 'preact/hooks';
import { api } from '../shared/api/reader';
import type { BookSummary, ReadingSettings } from '../shared/contracts/book';
import { LibraryPage } from '../features/library/LibraryPage';
import { ReaderPage } from '../features/reader/ReaderPage';
import { SettingsPage } from '../features/settings/SettingsPage';
import appIcon from '../../src-tauri/icons/32x32.png';

const defaults: ReadingSettings = { fontSize: 19, fontColor: null, lineHeight: 1.8, theme: 'paper', sidebarCollapsed: false, pdfZoom: 1, alwaysOnTop: false, opacity: 1, previousPageShortcut: 'ArrowLeft', nextPageShortcut: 'ArrowRight', windowShortcutMode: 'double' };
const compactWidth = 1100;

export function App() {
  const [route, setRoute] = useState<'library' | 'reader' | 'settings'>('library');
  const [book, setBook] = useState<BookSummary | null>(null);
  const [settings, setSettings] = useState<ReadingSettings>(() => ({ ...defaults, sidebarCollapsed: window.innerWidth <= compactWidth }));
  const settingsRef = useRef(settings);
  const saveQueue = useRef<Promise<void>>(Promise.resolve());
  const sidebarRef = useRef<HTMLElement>(null);
  const [shortcutError, setShortcutError] = useState<string | null>(null);

  useEffect(() => {
    api.getReadingSettings().then(value => {
      if (window.innerWidth <= compactWidth && !value.sidebarCollapsed) saveSettings({ ...value, sidebarCollapsed: true });
      else { settingsRef.current = value; setSettings(value); }
    }).catch(() => {});
    api.getShortcutStatus().then(setShortcutError).catch(() => {});
  }, []);

  function saveSettings(next: ReadingSettings) {
    settingsRef.current = next;
    setSettings(next);
    saveQueue.current = saveQueue.current.then(() => api.setReadingSettings(next)).then(() => {}, error => window.alert(String(error?.message || error)));
  }

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.ctrlKey && !event.altKey && !event.metaKey && event.key.toLowerCase() === 'b') {
        event.preventDefault();
        saveSettings({ ...settingsRef.current, sidebarCollapsed: !settingsRef.current.sidebarCollapsed });
      }
    }
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  useEffect(() => {
    function onResize() {
      if (window.innerWidth <= compactWidth && !settingsRef.current.sidebarCollapsed) saveSettings({ ...settingsRef.current, sidebarCollapsed: true });
    }
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);

  function openBook(next: BookSummary) { setBook(current => current?.id === next.id ? { ...next, position: current.position } : next); setRoute('reader'); }
  function library() { setRoute('library'); }

  return <div class={`app-shell theme-${settings.theme} route-${route} ${settings.sidebarCollapsed ? 'sidebar-collapsed' : ''} ${settings.fontColor && route !== 'settings' ? 'custom-font-color' : ''}`} style={{ '--window-opacity': String(settings.opacity), '--app-font-color': route === 'settings' ? undefined : settings.fontColor ?? undefined }} onClick={event => {
    if (!settingsRef.current.sidebarCollapsed && event.target instanceof Node && !sidebarRef.current?.contains(event.target)) saveSettings({ ...settingsRef.current, sidebarCollapsed: true });
  }}>
    <header class="app-titlebar" data-tauri-drag-region>
      <span data-tauri-drag-region>SSReader</span>
      {settings.sidebarCollapsed && <button class="sidebar-open-button" aria-label="展开侧栏" aria-controls="main-sidebar" aria-expanded={false} title="展开侧栏 (Ctrl+B)" onClick={event => { event.stopPropagation(); saveSettings({ ...settingsRef.current, sidebarCollapsed: false }); }}><img src={appIcon} alt="" /></button>}
    </header>
    <aside class="sidebar" id="main-sidebar" ref={sidebarRef}>
      <nav><button title="我的书架" aria-label="我的书架" class={route === 'library' ? 'nav-item active' : 'nav-item'} onClick={library}><span aria-hidden="true">▤</span><span class="nav-label">我的书架</span></button>
        {book && <button title="阅读模式" aria-label="阅读模式" class={route === 'reader' ? 'nav-item active' : 'nav-item'} onClick={() => setRoute('reader')}><span aria-hidden="true">▧</span><span class="nav-label">阅读模式</span></button>}
        <button title="设置" aria-label="设置" class={route === 'settings' ? 'nav-item active' : 'nav-item'} onClick={() => setRoute('settings')}><span aria-hidden="true">⚙</span><span class="nav-label">设置</span></button></nav>
      <div class="sidebar-footer"><p>本地使用 · 无需登录</p></div>
    </aside>
    <div class="content-area">
      {shortcutError && <div class="shortcut-warning" role="status">{shortcutError}。可通过系统托盘打开主窗口。<button class="text-button" aria-label="关闭快捷键提示" onClick={() => setShortcutError(null)}>知道了</button></div>}
      {route === 'library' && <LibraryPage onOpen={openBook} onSettings={() => setRoute('settings')} />}
      {route === 'reader' && book && <ReaderPage key={book.id} book={book} settings={settings} onSettingsChange={saveSettings} onPositionChange={position => setBook(current => current?.id === book.id ? { ...current, position } : current)} />}
      {route === 'settings' && <SettingsPage settings={settings} onChange={saveSettings} onBack={library} onRestored={() => { setBook(null); setRoute('library'); }} />}
    </div>
  </div>;
}
