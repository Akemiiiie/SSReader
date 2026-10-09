import { TextLayer, version, type PDFDocumentProxy, type PDFPageProxy, type RenderTask } from 'pdfjs-dist';
import { useEffect, useLayoutEffect, useRef, useState } from 'preact/hooks';
import { api, asAppError } from '../../shared/api/reader';
import type { Anchor, BookSummary, PdfBlock, PdfPageText, ReadingSettings, SelectionSnapshot } from '../../shared/contracts/book';
import { openPdfBytes } from './pdf/pdfDocument';
import { alignCodePoint, codePointOffsetAtY, turnWithinViewport } from './pagePosition';
import { sentenceContext } from './selectionContext';
import 'pdfjs-dist/web/pdf_viewer.css';

const extractorVersion = `pdfjs-${version}`;

export function PdfReader({ book, settings, jump, onAnchor, onSelection, onZoom, previousShortcut, nextShortcut }: { book: BookSummary; settings: ReadingSettings; jump: Anchor | null; onAnchor: (anchor: Anchor) => void; onSelection: (snapshot: SelectionSnapshot) => void; onZoom: (zoom: number) => void; previousShortcut: string; nextShortcut: string }) {
  const initial = book.position?.kind === 'pdf' ? book.position : null;
  const [document, setDocument] = useState<PDFDocumentProxy | null>(null);
  const [pageNo, setPageNo] = useState(initial?.pageNo ?? 1);
  const [mode, setMode] = useState<'original' | 'reflow'>(initial?.mode ?? 'reflow');
  const [artifact, setArtifact] = useState<PdfPageText | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const canvas = useRef<HTMLCanvasElement>(null);
  const layer = useRef<HTMLDivElement>(null);
  const scroll = useRef<HTMLDivElement>(null);
  const restoreAnchor = useRef<Anchor | null>(initial);
  const restoreToEnd = useRef(false);
  const reportOnLeave = useRef<() => void>(() => {});
  const pageAnchor = useRef<Anchor | null>(initial);

  useEffect(() => {
    const report = () => reportOnLeave.current();
    const onVisibility = () => { if (window.document.visibilityState === 'hidden') report(); };
    window.addEventListener('blur', report);
    window.document.addEventListener('visibilitychange', onVisibility);
    return () => {
      report();
      window.removeEventListener('blur', report);
      window.document.removeEventListener('visibilitychange', onVisibility);
    };
  }, []);

  function currentAnchor(): Anchor {
    const container = scroll.current;
    const effectiveMode = artifact?.quality === 'no-text' ? 'original' : mode;
    if (effectiveMode === 'reflow' && container) {
      const top = container.getBoundingClientRect().top + 1;
      const blocks = Array.from(container.querySelectorAll<HTMLElement>('[data-block]'));
      const block = blocks.find(element => element.getBoundingClientRect().bottom > top) || blocks[blocks.length - 1];
      if (block) return { kind: 'pdf', version: 1, pageNo, mode: 'reflow', blockId: block.dataset.block, codePointOffset: codePointOffsetAtY(block, top) };
    }
    const maximum = container ? Math.max(0, container.scrollHeight - container.clientHeight) : 0;
    return { kind: 'pdf', version: 1, pageNo, mode: effectiveMode, scrollRatio: maximum && container ? container.scrollTop / maximum : 0 };
  }

  function reportPosition() {
    if (!loading && artifact?.pageNo === pageNo) {
      const anchor = currentAnchor();
      pageAnchor.current = anchor;
      onAnchor(anchor);
    }
  }
  reportOnLeave.current = reportPosition;

  function goToPage(nextPage: number, end = false) {
    if (nextPage === pageNo) return;
    restoreToEnd.current = end;
    setPageNo(nextPage);
    onAnchor({ kind: 'pdf', version: 1, pageNo: nextPage, mode: artifact?.quality === 'no-text' ? 'original' : mode });
  }

  function turn(direction: 'previous' | 'next') {
    const container = scroll.current;
    if (!container || loading || !artifact || artifact.pageNo !== pageNo) return;
    if (direction === 'previous') {
      if (turnWithinViewport(container, null, 'previous', settings.fontSize * settings.lineHeight)) reportPosition();
      else goToPage(Math.max(1, pageNo - 1), true);
      return;
    }
    const effectiveMode = artifact.quality === 'no-text' ? 'original' : mode;
    const blocks = container.querySelectorAll<HTMLElement>('[data-block]');
    const lastBlock = blocks[blocks.length - 1];
    const contentEnd = effectiveMode === 'original' ? canvas.current : lastBlock;
    if (turnWithinViewport(container, contentEnd || null, 'next', settings.fontSize * settings.lineHeight)) reportPosition();
    else goToPage(Math.min(document?.numPages || book.pageCount || 1, pageNo + 1));
  }

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.repeat || event.ctrlKey || event.altKey || event.metaKey || event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLSelectElement || (event.target instanceof HTMLElement && event.target.isContentEditable)) return;
      if (event.key === previousShortcut) { event.preventDefault(); turn('previous'); }
      else if (event.key === nextShortcut) { event.preventDefault(); turn('next'); }
    }
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [previousShortcut, nextShortcut, document, book.pageCount, artifact, loading, mode, pageNo, settings.fontSize, settings.lineHeight]);

  useEffect(() => {
    if (jump?.kind === 'pdf') { restoreAnchor.current = jump; setPageNo(jump.pageNo); setMode(jump.mode); }
  }, [jump]);

  useEffect(() => {
    let disposed = false;
    let task: ReturnType<typeof openPdfBytes> | null = null;
    api.getPdfMode(book.id).then(saved => { if (!disposed && !initial) setMode(saved); }).catch(() => {});
    api.getPdfBytes(book.id).then(bytes => {
      if (disposed) return;
      task = openPdfBytes(bytes);
      return task.promise;
    }).then(pdf => { if (pdf && !disposed) { setDocument(pdf); setLoading(false); } else pdf?.destroy(); })
      .catch(error => { if (!disposed) { setError(asAppError(error).message); setLoading(false); } });
    return () => { disposed = true; task?.destroy(); };
  }, [book.id]);

  useEffect(() => {
    if (!document) return;
    let cancelled = false;
    let page: PDFPageProxy | null = null;
    let rendering: RenderTask | null = null;
    let textLayer: TextLayer | null = null;
    setLoading(true); setArtifact(null); setError(null);

    async function load() {
      try {
        page = await document!.getPage(pageNo);
        const cached = await api.getPdfPageText(book.id, pageNo);
        let result = cached?.extractorVersion === extractorVersion ? cached : null;
        const content = await page.getTextContent();
        if (!result) {
          const blocks: PdfBlock[] = content.items.filter(item => 'str' in item && item.str.trim()).map((item, index) => {
            const text = item as { str: string; transform: number[]; width: number; height: number };
            return { id: `${pageNo}:${index}`, text: text.str, x: text.transform[4], y: text.transform[5], width: text.width, height: text.height };
          });
          result = { pageNo, quality: blocks.length ? 'uncertain' : 'no-text', blocks, extractorVersion };
          await api.savePdfPageText(book.id, pageNo, result.quality, blocks, extractorVersion);
        }
        if (cancelled) return;
        setArtifact(result);
        const actualMode = result.quality === 'no-text' ? 'original' : mode;
        if (actualMode === 'original' && canvas.current && layer.current) {
          const viewport = page.getViewport({ scale: settings.pdfZoom });
          const target = canvas.current;
          const context = target.getContext('2d');
          if (!context) throw new Error('画布不可用');
          const ratio = window.devicePixelRatio || 1;
          target.width = Math.ceil(viewport.width * ratio); target.height = Math.ceil(viewport.height * ratio);
          target.style.width = `${viewport.width}px`; target.style.height = `${viewport.height}px`;
          context.setTransform(ratio, 0, 0, ratio, 0, 0);
          rendering = page.render({ canvas: target, canvasContext: context, viewport });
          await rendering.promise;
          if (cancelled) return;
          layer.current.replaceChildren();
          layer.current.style.width = `${viewport.width}px`; layer.current.style.height = `${viewport.height}px`;
          layer.current.style.setProperty('--scale-factor', String(viewport.scale));
          textLayer = new TextLayer({ textContentSource: content, container: layer.current, viewport });
          await textLayer.render();
        }
        if (!cancelled) setLoading(false);
      } catch (error) {
        if (!cancelled) { setError(asAppError(error).message); setLoading(false); }
      }
    }
    load();
    return () => { cancelled = true; rendering?.cancel(); textLayer?.cancel(); page?.cleanup(); if (layer.current) layer.current.replaceChildren(); };
  }, [document, pageNo, mode, settings.pdfZoom]);

  useLayoutEffect(() => {
    const container = scroll.current;
    if (loading || !container || !artifact || artifact.pageNo !== pageNo) return;
    const saved = restoreAnchor.current;
    restoreAnchor.current = null;
    if (restoreToEnd.current) {
      container.scrollTop = 0;
      const blocks = container.querySelectorAll<HTMLElement>('[data-block]');
      const contentEnd = artifact.quality === 'no-text' || mode === 'original' ? canvas.current : blocks[blocks.length - 1];
      if (contentEnd) container.scrollTop = Math.max(0, contentEnd.getBoundingClientRect().bottom - container.getBoundingClientRect().bottom);
      restoreToEnd.current = false;
    } else if (saved?.kind === 'pdf' && saved.pageNo === pageNo && saved.mode === (artifact.quality === 'no-text' ? 'original' : mode)) {
      if (saved.mode === 'original') {
        container.scrollTop = (container.scrollHeight - container.clientHeight) * (saved.scrollRatio ?? 0);
      } else {
        const block = Array.from(container.querySelectorAll<HTMLElement>('[data-block]')).find(element => element.dataset.block === saved.blockId);
        if (block) alignCodePoint(container, block, saved.codePointOffset ?? 0);
        else container.scrollTop = 0;
      }
    } else container.scrollTop = 0;
    reportPosition();
  }, [loading, artifact, pageNo, mode]);

  useLayoutEffect(() => {
    const container = scroll.current;
    if (!container || loading || !artifact || artifact.pageNo !== pageNo) return;
    const observer = new ResizeObserver(() => {
      const saved = pageAnchor.current;
      if (saved?.kind !== 'pdf' || saved.pageNo !== pageNo) return;
      if (saved.mode === 'original') container.scrollTop = (container.scrollHeight - container.clientHeight) * (saved.scrollRatio ?? 0);
      else {
        const block = Array.from(container.querySelectorAll<HTMLElement>('[data-block]')).find(element => element.dataset.block === saved.blockId);
        if (block) alignCodePoint(container, block, saved.codePointOffset ?? 0);
      }
      reportPosition();
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, [loading, artifact, pageNo, mode, settings.fontSize, settings.lineHeight]);

  function changeMode(next: 'original' | 'reflow') {
    setMode(next);
    onAnchor({ kind: 'pdf', version: 1, pageNo, mode: next });
    api.setPdfMode(book.id, next).catch(error => setError(asAppError(error).message));
  }

  function updateZoom(value: number) {
    restoreAnchor.current = currentAnchor();
    onZoom(value);
  }

  async function fitPage(direction: 'width' | 'page') {
    if (!document || !scroll.current) return;
    try {
      const page = await document.getPage(pageNo);
      const viewport = page.getViewport({ scale: 1 });
      const widthScale = (scroll.current.clientWidth - 48) / viewport.width;
      const heightScale = (scroll.current.clientHeight - 48) / viewport.height;
      updateZoom(Math.max(0.25, Math.min(3, direction === 'width' ? widthScale : Math.min(widthScale, heightScale))));
      changeMode('original');
    } catch (error) { setError(asAppError(error).message); }
  }

  function selected() {
    const selection = window.getSelection();
    const text = selection?.toString().trim();
    if (!text) return;
    const node = selection?.anchorNode instanceof Element ? selection.anchorNode : selection?.anchorNode?.parentElement;
    const blockId = node?.closest<HTMLElement>('[data-block]')?.dataset.block;
    const block = node?.closest<HTMLElement>('[data-block]');
    let context = sentenceContext(text, 0);
    if (block && selection?.rangeCount && block.contains(selection.getRangeAt(0).startContainer)) {
      const prefix = window.document.createRange();
      prefix.selectNodeContents(block);
      prefix.setEnd(selection.getRangeAt(0).startContainer, selection.getRangeAt(0).startOffset);
      context = sentenceContext(block.textContent ?? text, prefix.toString().length);
    }
    onSelection({ bookId: book.id, format: 'pdf', text, sourceLabel: `第 ${pageNo} 页`, ...context, anchor: { kind: 'pdf', version: 1, pageNo, mode: artifact?.quality === 'no-text' ? 'original' : mode, blockId } });
  }

  const effectiveMode = artifact?.quality === 'no-text' ? 'original' : mode;
  return <div class="reader-body pdf-reader" onMouseUp={selected}>
    <div class="reader-subtoolbar">
      <span class="toolbar-spacer" />
      <button class={effectiveMode === 'reflow' ? 'segmented active' : 'segmented'} onClick={() => changeMode('reflow')} disabled={artifact?.quality === 'no-text'}>重排文本</button>
      <button class={effectiveMode === 'original' ? 'segmented active' : 'segmented'} onClick={() => changeMode('original')}>原页</button>
      <button class="text-button" onClick={() => fitPage('width')}>适宽</button><button class="text-button" onClick={() => fitPage('page')}>适页</button>
      <button class="text-button" onClick={() => updateZoom(Math.max(0.25, settings.pdfZoom - 0.1))}>－</button><span>{Math.round(settings.pdfZoom * 100)}%</span><button class="text-button" onClick={() => updateZoom(Math.min(3, settings.pdfZoom + 0.1))}>＋</button>
    </div>
    {error && <p class="inline-notice" role="alert">{error}</p>}
    {artifact?.quality === 'no-text' && <p class="inline-notice">当前页无可选文字，OCR 留待后续版本。仍可按原页阅读。</p>}
    {artifact?.quality === 'uncertain' && effectiveMode === 'reflow' && <p class="inline-notice">文本顺序可能受多栏、公式或图表影响，请随时切回原页核对。</p>}
    <div class="reading-scroll pdf-scroll" ref={scroll}>
      {loading && <p class="loading-overlay">正在加载第 {pageNo} 页…</p>}
      <div class="pdf-page" style={{ display: effectiveMode === 'original' ? 'block' : 'none' }}><canvas ref={canvas} /><div class="textLayer" ref={layer} /></div>
      {effectiveMode === 'reflow' && <article class="reflow-content" style={{ fontSize: `${settings.fontSize}px`, lineHeight: settings.lineHeight }}>
        {artifact?.blocks.map(block => <span data-block={block.id} key={block.id}>{block.text}{' '}</span>)}
        {artifact && <div class="page-boundary">— 第 {pageNo} 页 — <button class="text-button" onClick={() => changeMode('original')}>查看原页</button></div>}
      </article>}
    </div>
    <div class="pager"><label>第 <input class="page-input" type="number" min="1" max={document?.numPages || book.pageCount || 1} value={pageNo} onChange={event => goToPage(Math.max(1, Math.min(document?.numPages || book.pageCount || 1, Number(event.currentTarget.value) || 1)))} /> / {document?.numPages || book.pageCount || '?'} 页</label></div>
  </div>;
}
