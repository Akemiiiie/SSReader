import { useEffect, useRef, useState } from 'preact/hooks';
import { api, asAppError } from '../../shared/api/reader';
import type { Anchor, BookSummary, ReadingSettings, SelectionSnapshot } from '../../shared/contracts/book';
import { TxtReader } from './TxtReader';
import { PdfReader } from './PdfReader';
import { AnalysisPopupController } from './AnalysisPopupController';

export function ReaderPage({ book, settings, compact = false, onSettingsChange, onPositionChange }: { compact?: boolean; book: BookSummary; settings: ReadingSettings; onSettingsChange: (value: ReadingSettings) => void; onPositionChange: (value: Anchor) => void }) {
  const latest = useRef<Anchor | null>(book.position);
  const pendingSave = useRef<Promise<void>>(Promise.resolve());
  const popup = useRef(new AnalysisPopupController());
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

  useEffect(() => {
    const dismiss = () => { void popup.current.dismiss(); };
    document.addEventListener('pointerdown', dismiss);
    return () => { document.removeEventListener('pointerdown', dismiss); void popup.current.dispose(); };
  }, []);

  function selected(snapshot: SelectionSnapshot) {
    void popup.current.open({ selection: snapshot, theme: settings.theme, fontColor: settings.fontColor, opacity: settings.opacity })
      .catch(error => setNotice(asAppError(error).message));
  }

  return <main class="reader-page">
    {saveError && <p class="inline-notice error" role="alert">阅读位置未保存：{saveError} <button class="text-button" onClick={() => { void flush(); }}>重试</button></p>}
    {notice && <p class="inline-notice" role="status">{notice} <button class="text-button" onClick={() => setNotice(null)}>关闭</button></p>}
    <div class="reader-layout">
      {book.format === 'txt'
        ? <TxtReader compact={compact} book={book} settings={settings} jump={null} onAnchor={current} onSelection={selected} previousShortcut={settings.previousPageShortcut} nextShortcut={settings.nextPageShortcut} />
        : <PdfReader book={book} settings={settings} jump={null} onAnchor={current} onSelection={selected} onZoom={pdfZoom => onSettingsChange({ ...settings, pdfZoom })} previousShortcut={settings.previousPageShortcut} nextShortcut={settings.nextPageShortcut} />}
    </div>
  </main>;
}
