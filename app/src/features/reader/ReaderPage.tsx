import { useEffect, useRef, useState } from 'preact/hooks';
import { api, asAppError } from '../../shared/api/reader';
import type { Anchor, BookSummary, ReadingSettings, SelectionSnapshot } from '../../shared/contracts/book';
import { TxtReader } from './TxtReader';
import { PdfReader } from './PdfReader';

export function ReaderPage({ book, settings, onSettingsChange, onPositionChange }: { book: BookSummary; settings: ReadingSettings; onSettingsChange: (value: ReadingSettings) => void; onPositionChange: (value: Anchor) => void }) {
  const latest = useRef<Anchor | null>(book.position);
  const pendingSave = useRef<Promise<void>>(Promise.resolve());
  const [selection, setSelection] = useState<SelectionSnapshot | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  function flush(value = latest.current) {
    if (!value) return Promise.resolve();
    pendingSave.current = pendingSave.current.then(() => api.savePosition(book.id, value)).then(
      () => setSaveError(null),
      error => setSaveError(asAppError(error).message),
    );
    return pendingSave.current;
  }
  function current(value: Anchor) {
    if (JSON.stringify(value) === JSON.stringify(latest.current)) return;
    latest.current = value;
    onPositionChange(value);
    void flush(value);
  }
  useEffect(() => {
    const onBlur = () => { void flush(); };
    const onVisibility = () => { if (document.visibilityState === 'hidden') void flush(); };
    window.addEventListener('blur', onBlur);
    document.addEventListener('visibilitychange', onVisibility);
    return () => { onBlur(); window.removeEventListener('blur', onBlur); document.removeEventListener('visibilitychange', onVisibility); };
  }, [book.id]);

  return <main class="reader-page">
    {saveError && <p class="inline-notice error" role="alert">阅读位置未保存：{saveError} <button class="text-button" onClick={() => { void flush(); }}>重试</button></p>}
    {notice && <p class="inline-notice" role="status">{notice} <button class="text-button" onClick={() => setNotice(null)}>关闭</button></p>}
    <div class="reader-layout">
      {book.format === 'txt'
        ? <TxtReader book={book} settings={settings} jump={null} onAnchor={current} onSelection={setSelection} previousShortcut={settings.previousPageShortcut} nextShortcut={settings.nextPageShortcut} />
        : <PdfReader book={book} settings={settings} jump={null} onAnchor={current} onSelection={setSelection} onZoom={pdfZoom => onSettingsChange({ ...settings, pdfZoom })} previousShortcut={settings.previousPageShortcut} nextShortcut={settings.nextPageShortcut} />}
    </div>
    {selection && <div class="selection-card" role="dialog" aria-label="原文选区"><div><strong>原文选区</strong><span>{selection.sourceLabel}</span></div><p>{selection.text}</p><div class="action-row"><button onClick={() => { navigator.clipboard.writeText(selection.text).catch(error => setNotice(String(error))); setSelection(null); }}>复制原文</button><button class="secondary" onClick={() => setSelection(null)}>关闭</button></div></div>}
  </main>;
}
