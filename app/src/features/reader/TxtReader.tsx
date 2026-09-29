import { useEffect, useRef, useState } from 'preact/hooks';
import { api, asAppError } from '../../shared/api/reader';
import type { Anchor, BookSummary, ReadingSettings, SelectionSnapshot, TxtChunk } from '../../shared/contracts/book';
import { codePointOffsetAtY, codePointOffsetStartingAtY } from './pagePosition';

type Cursor = { paragraphIndex: number; codePointOffset: number };
type Page = { start: Cursor; end: Cursor | null; paragraphs: TxtChunk[] };
type Buffer = { start: Cursor; chunks: TxtChunk[]; next: Cursor | null };

function compareCursor(left: Cursor, right: Cursor): number {
  return left.paragraphIndex - right.paragraphIndex || left.codePointOffset - right.codePointOffset;
}

function pointLength(text: string): number {
  return Array.from(text).length;
}

function groupParagraphs(chunks: TxtChunk[]): TxtChunk[] {
  const paragraphs: TxtChunk[] = [];
  for (const chunk of chunks) {
    const previous = paragraphs[paragraphs.length - 1];
    if (previous?.paragraphIndex === chunk.paragraphIndex && previous.codePointOffset + pointLength(previous.text) === chunk.codePointOffset) {
      previous.text += chunk.text;
    } else paragraphs.push({ ...chunk });
  }
  return paragraphs;
}

function chunksFrom(chunks: TxtChunk[], start: Cursor): TxtChunk[] {
  const remaining: TxtChunk[] = [];
  for (const chunk of chunks) {
    if (chunk.paragraphIndex < start.paragraphIndex) continue;
    if (chunk.paragraphIndex > start.paragraphIndex) { remaining.push(chunk); continue; }
    const length = pointLength(chunk.text);
    if (chunk.codePointOffset + length < start.codePointOffset || (length > 0 && chunk.codePointOffset + length === start.codePointOffset)) continue;
    const skipped = Math.max(0, start.codePointOffset - chunk.codePointOffset);
    remaining.push({ ...chunk, codePointOffset: chunk.codePointOffset + skipped, text: Array.from(chunk.text).slice(skipped).join('') });
  }
  return remaining;
}

function paragraphsTo(paragraphs: TxtChunk[], end: Cursor | null): TxtChunk[] {
  if (!end) return paragraphs;
  const page: TxtChunk[] = [];
  for (const paragraph of paragraphs) {
    const start = { paragraphIndex: paragraph.paragraphIndex, codePointOffset: paragraph.codePointOffset };
    if (compareCursor(start, end) >= 0) break;
    if (paragraph.paragraphIndex === end.paragraphIndex) {
      page.push({ ...paragraph, text: Array.from(paragraph.text).slice(0, end.codePointOffset - paragraph.codePointOffset).join('') });
      break;
    }
    page.push(paragraph);
  }
  return page;
}

function measureParagraphs(measure: HTMLElement, paragraphs: TxtChunk[]): HTMLElement[] {
  const elements = paragraphs.map(paragraph => {
    const element = document.createElement('p');
    element.textContent = paragraph.text || '\u00a0';
    return element;
  });
  measure.replaceChildren(...elements);
  return elements;
}

function pageBreak(paragraphs: TxtChunk[], elements: HTMLElement[], bottom: number): Cursor | null {
  for (let index = 0; index < paragraphs.length; index++) {
    const paragraph = paragraphs[index];
    const element = elements[index];
    if (element.getBoundingClientRect().bottom <= bottom) continue;
    const offset = codePointOffsetAtY(element, bottom);
    if (offset >= pointLength(paragraph.text) && paragraph.text) continue;
    return { paragraphIndex: paragraph.paragraphIndex, codePointOffset: paragraph.codePointOffset + Math.min(offset, pointLength(paragraph.text)) };
  }
  return null;
}

export function TxtReader({ book, settings, jump, onAnchor, onSelection, previousShortcut, nextShortcut }: { book: BookSummary; settings: ReadingSettings; jump: Anchor | null; onAnchor: (anchor: Anchor) => void; onSelection: (snapshot: SelectionSnapshot) => void; previousShortcut: string; nextShortcut: string }) {
  const initial = book.position?.kind === 'txt' ? book.position : null;
  const [start, setStart] = useState<Cursor>({ paragraphIndex: initial?.paragraphIndex ?? 0, codePointOffset: initial?.codePointOffset ?? 0 });
  const [page, setPage] = useState<Page | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [layoutVersion, setLayoutVersion] = useState(0);
  const viewport = useRef<HTMLDivElement>(null);
  const measure = useRef<HTMLElement | null>(null);
  const buffer = useRef<Buffer | null>(null);
  const history = useRef<Cursor[]>([]);
  const turning = useRef(false);
  const mounted = useRef(true);
  const onAnchorRef = useRef(onAnchor);
  onAnchorRef.current = onAnchor;

  useEffect(() => {
    const element = document.createElement('article');
    element.className = 'txt-content txt-measure';
    document.body.appendChild(element);
    measure.current = element;
    const container = viewport.current;
    let width = container?.clientWidth ?? 0;
    let height = container?.clientHeight ?? 0;
    const observer = new ResizeObserver(() => {
      if (!container || (width === container.clientWidth && height === container.clientHeight)) return;
      width = container.clientWidth;
      height = container.clientHeight;
      setLayoutVersion(value => value + 1);
    });
    if (container) observer.observe(container);
    return () => { mounted.current = false; observer.disconnect(); element.remove(); measure.current = null; };
  }, []);

  useEffect(() => {
    if (jump?.kind !== 'txt') return;
    history.current = [];
    buffer.current = null;
    setPage(null);
    setLayoutVersion(value => value + 1);
    setStart({ paragraphIndex: jump.paragraphIndex, codePointOffset: jump.codePointOffset });
  }, [jump]);

  useEffect(() => { history.current = []; }, [settings.fontSize, settings.lineHeight, layoutVersion]);

  useEffect(() => {
    const container = viewport.current;
    const element = measure.current;
    if (!container || !element) return;
    let active = true;
    turning.current = true;
    element.style.width = `${Math.min(780, container.clientWidth)}px`;
    element.style.fontSize = `${settings.fontSize}px`;
    element.style.lineHeight = String(settings.lineHeight);
    const bottom = element.getBoundingClientRect().top + container.clientHeight - (parseFloat(getComputedStyle(element).paddingBottom) || 0);

    async function loadPage() {
      try {
        const cached = buffer.current;
        let chunks: TxtChunk[] = [];
        let next: Cursor | null = start;
        if (cached && compareCursor(start, cached.start) >= 0 && (!cached.next || compareCursor(start, cached.next) < 0)) {
          chunks = chunksFrom(cached.chunks, start);
          next = cached.next;
        }
        while (active) {
          if (!chunks.length && next) {
            const slice = await api.getTxtSlice(book.id, next.paragraphIndex, next.codePointOffset);
            if (!active) return;
            chunks = slice.chunks;
            next = slice.nextParagraphIndex === null ? null : { paragraphIndex: slice.nextParagraphIndex, codePointOffset: slice.nextCodePointOffset ?? 0 };
          }
          const paragraphs = groupParagraphs(chunks);
          const elements = measureParagraphs(element!, paragraphs);
          const end = pageBreak(paragraphs, elements, bottom);
          if (end && compareCursor(end, start) === 0) {
            setPage(null); setError('阅读区域过小，无法完整显示一行文字。请放大窗口或调小字体。');
            break;
          }
          if (end || !next) {
            if (!active) return;
            buffer.current = { start, chunks, next };
            setPage({ start, end, paragraphs: paragraphsTo(paragraphs, end) });
            setError(null);
            if (paragraphs[0]) onAnchorRef.current({ kind: 'txt', version: 1, paragraphIndex: start.paragraphIndex, codePointOffset: start.codePointOffset, paragraphFingerprint: paragraphs[0].fingerprint });
            break;
          }
          const slice = await api.getTxtSlice(book.id, next.paragraphIndex, next.codePointOffset);
          if (!active) return;
          chunks = [...chunks, ...slice.chunks];
          next = slice.nextParagraphIndex === null ? null : { paragraphIndex: slice.nextParagraphIndex, codePointOffset: slice.nextCodePointOffset ?? 0 };
        }
      } catch (failure) {
        if (!active) return;
        const appError = asAppError(failure);
        if (appError.code === 'ANCHOR_STALE' && (start.paragraphIndex > 0 || start.codePointOffset > 0)) {
          buffer.current = null;
          setStart({ paragraphIndex: 0, codePointOffset: 0 });
          setError('旧位置已失效，已安全回到开头。');
        } else { setPage(null); setError(appError.message); }
      } finally { if (active) turning.current = false; }
    }
    void loadPage();
    return () => { active = false; };
  }, [book.id, start.paragraphIndex, start.codePointOffset, settings.fontSize, settings.lineHeight, layoutVersion]);

  async function findPrevious(current: Cursor): Promise<Cursor | null> {
    const container = viewport.current;
    const element = measure.current;
    if (!container || !element) return null;
    let before = current;
    let chunks: TxtChunk[] = [];
    for (let attempt = 0; attempt < 8; attempt++) {
      const slice = await api.getTxtSliceBefore(book.id, before.paragraphIndex, before.codePointOffset);
      if (!slice?.chunks.length) break;
      chunks = [...slice.chunks, ...chunks];
      const first = chunks[0];
      const firstCursor = { paragraphIndex: first.paragraphIndex, codePointOffset: first.codePointOffset };
      if (compareCursor(firstCursor, before) >= 0) break;
      before = firstCursor;
      const paragraphs = groupParagraphs(chunks);
      const elements = measureParagraphs(element, paragraphs);
      const style = getComputedStyle(element);
      const usableHeight = container.clientHeight - (parseFloat(style.paddingTop) || 0) - (parseFloat(style.paddingBottom) || 0);
      const threshold = elements[elements.length - 1].getBoundingClientRect().bottom - usableHeight;
      if (elements[0].getBoundingClientRect().top >= threshold && (before.paragraphIndex > 0 || before.codePointOffset > 0) && attempt < 7) continue;
      for (let index = 0; index < paragraphs.length; index++) {
        const paragraph = paragraphs[index];
        const element = elements[index];
        if (element.getBoundingClientRect().bottom <= threshold) continue;
        const offset = codePointOffsetStartingAtY(element, threshold);
        if (offset >= pointLength(paragraph.text) && paragraph.text) continue;
        const candidate = { paragraphIndex: paragraph.paragraphIndex, codePointOffset: paragraph.codePointOffset + Math.min(offset, pointLength(paragraph.text)) };
        if (compareCursor(candidate, current) < 0) return candidate;
      }
      return before;
    }
    return chunks.length ? before : null;
  }

  function next() {
    if (!page?.end || turning.current) return;
    turning.current = true;
    history.current.push(page.start);
    setPage(null);
    setStart(page.end);
  }

  function previous() {
    if (!page || turning.current) return;
    const cached = history.current.pop();
    if (cached) { turning.current = true; setPage(null); setStart(cached); return; }
    turning.current = true;
    void findPrevious(page.start).then(previousStart => {
      if (!mounted.current || !previousStart) return;
      setPage(null);
      setStart(previousStart);
    }).catch(failure => { if (mounted.current) setError(asAppError(failure).message); })
      .finally(() => { if (mounted.current) turning.current = false; });
  }

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.repeat || event.ctrlKey || event.altKey || event.metaKey || event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLSelectElement || (event.target instanceof HTMLElement && event.target.isContentEditable)) return;
      if (event.key === previousShortcut) { event.preventDefault(); previous(); }
      else if (event.key === nextShortcut) { event.preventDefault(); next(); }
    }
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [previousShortcut, nextShortcut, page]);

  function selected() {
    const selection = window.getSelection();
    const text = selection?.toString().trim();
    if (!text || !page) return;
    const range = selection!.getRangeAt(0);
    const node = range.startContainer instanceof Element ? range.startContainer : range.startContainer.parentElement;
    const element = node?.closest<HTMLElement>('[data-paragraph]');
    if (!element) return;
    const paragraph = page.paragraphs.find(item => item.paragraphIndex === Number(element.dataset.paragraph) && item.codePointOffset === Number(element.dataset.offset));
    if (!paragraph) return;
    const before = document.createRange();
    before.selectNodeContents(element);
    before.setEnd(range.startContainer, range.startOffset);
    const codePointOffset = paragraph.codePointOffset + pointLength(before.toString());
    onSelection({ bookId: book.id, format: 'txt', text, sourceLabel: `第 ${paragraph.paragraphIndex + 1} 段`, anchor: { kind: 'txt', version: 1, paragraphIndex: paragraph.paragraphIndex, codePointOffset, paragraphFingerprint: paragraph.fingerprint } });
  }

  return <div class="reader-body txt-reader">
    {error && <p class="inline-notice" role="alert">{error}</p>}
    <div class="reading-scroll txt-viewport" ref={viewport} onMouseUp={selected}>
      {!page && !error && <p class="state-card">正在排版正文…</p>}
      {page && <article class="txt-content" style={{ fontSize: `${settings.fontSize}px`, lineHeight: settings.lineHeight }}>
        {page.paragraphs.map(paragraph => <p key={`${paragraph.paragraphIndex}-${paragraph.codePointOffset}`} data-paragraph={paragraph.paragraphIndex} data-offset={paragraph.codePointOffset}>{paragraph.text || '\u00a0'}</p>)}
      </article>}
    </div>
  </div>;
}
