import { useEffect, useState } from 'preact/hooks';
import { api, asAppError } from '../../shared/api/reader';
import type { AppError, BookSummary, ImportSelection } from '../../shared/contracts/book';

type LibraryState = { kind: 'loading' } | { kind: 'ready'; books: BookSummary[] } | { kind: 'error'; error: AppError };

function progress(book: BookSummary): string {
  const anchor = book.position;
  if (!anchor) return '尚未开始';
  if (anchor.kind === 'pdf') return `第 ${anchor.pageNo} / ${book.pageCount ?? '?'} 页`;
  return `第 ${anchor.paragraphIndex + 1} / ${book.paragraphCount ?? '?'} 段`;
}

export function LibraryPage({ onOpen, onSettings }: { onOpen: (book: BookSummary) => void; onSettings: () => void }) {
  const [state, setState] = useState<LibraryState>({ kind: 'loading' });
  const [selection, setSelection] = useState<ImportSelection | null>(null);
  const [busy, setBusy] = useState(false);
  const [previewBusy, setPreviewBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  function refresh() {
    setState({ kind: 'loading' });
    api.listBooks().then(books => setState({ kind: 'ready', books }), error => setState({ kind: 'error', error: asAppError(error) }));
  }
  useEffect(refresh, []);

  async function selectFile() {
    try { setNotice(null); setSelection(await api.selectImportFile()); }
    catch (error) { setNotice(asAppError(error).message); }
  }

  async function importFile() {
    if (!selection || previewBusy) return;
    setBusy(true);
    try {
      const result = await api.importBook(selection.token, selection.txtEncoding);
      setSelection(null);
      onOpen(result.book);
      if (result.duplicate) setNotice('已打开相同内容的现有书籍');
    } catch (error) { setNotice(asAppError(error).message); }
    finally { setBusy(false); }
  }

  async function changeEncoding(encoding: string) {
    if (!selection || selection.format !== 'txt') return;
    const token = selection.token;
    setPreviewBusy(true);
    try {
      const preview = await api.previewTxtImport(token, encoding);
      setSelection(current => current?.token === token ? { ...current, txtEncoding: preview.encoding, preview: preview.text, encodingUncertain: false } : current);
      setNotice(null);
    } catch (error) { setNotice(asAppError(error).message); }
    finally { setPreviewBusy(false); }
  }

  async function action(book: BookSummary, kind: 'rename' | 'archive' | 'delete' | 'export') {
    try {
      if (kind === 'rename') {
        const title = window.prompt('书名', book.title);
        if (title === null) return;
        const author = window.prompt('作者（可留空）', book.author || '');
        if (author === null) return;
        await api.updateBook(book.id, title, author);
      } else if (kind === 'archive') {
        if (!window.confirm(`归档《${book.title}》？原文件、书签和位置会保留。`)) return;
        await api.archiveBook(book.id);
      } else if (kind === 'delete') {
        if (!window.confirm(`彻底删除《${book.title}》及受管原文件、位置和书签？此操作无法撤销。`)) return;
        await api.deleteBook(book.id);
      } else {
        await api.exportBook(book.id);
      }
      refresh();
    } catch (error) { setNotice(asAppError(error).message); }
  }

  const books = state.kind === 'ready' ? state.books : [];
  return (
    <main class="page library-page">
      <header class="page-header">
        <div><p class="eyebrow">LOCAL LIBRARY</p><h1>我的书架</h1><p class="muted">书籍与阅读记录只保存在这台设备上。</p></div>
        <div class="header-actions"><button class="secondary" onClick={onSettings}>设置与备份</button><button onClick={selectFile}>导入 TXT / PDF</button></div>
      </header>
      {notice && <p class="inline-notice" role="alert">{notice} <button class="text-button" onClick={() => setNotice(null)}>关闭</button></p>}
      {selection && <section class="import-card" aria-label="待导入文件">
        <strong>{selection.fileName}</strong><span>{selection.format.toUpperCase()} · {(selection.byteSize / 1024 / 1024).toFixed(2)} MiB</span>
        {selection.format === 'txt' && <div class="import-text-preview">
          <label>文本编码 <select value={selection.txtEncoding || 'utf-8'} disabled={busy || previewBusy} onChange={event => changeEncoding(event.currentTarget.value)}><option value="utf-8">UTF-8</option><option value="utf-16le">UTF-16 LE</option><option value="utf-16be">UTF-16 BE</option><option value="gb18030">GB18030</option></select></label>
          {selection.encodingUncertain && <p class="warning">编码自动识别不确定，请核对预览；如有乱码，可手动改选。</p>}
          <pre class="import-preview">{selection.preview}</pre>
        </div>}
        <div class="action-row"><button onClick={importFile} disabled={busy || previewBusy}>{busy ? '正在校验并导入…' : '开始导入'}</button><button class="secondary" onClick={async () => { try { await api.cancelImport(selection.token); setSelection(null); } catch (error) { setNotice(asAppError(error).message); } }}>取消导入</button></div>
      </section>}
      {state.kind === 'loading' && <section class="state-card" role="status">正在读取书架…</section>}
      {state.kind === 'error' && <section class="state-card error" role="alert"><h2>书架暂时无法读取</h2><p>{state.error.message}</p><p class="muted">错误代码：{state.error.code}。现有数据不会自动重置。</p><div class="action-row"><button onClick={refresh}>重试</button><button class="secondary" onClick={onSettings}>从备份恢复</button></div></section>}
      {state.kind === 'ready' && books.length === 0 && <section class="state-card"><h2>书架还是空的</h2><p>选择本地 TXT 或 PDF 文件，开始离线阅读。</p><button onClick={selectFile}>导入第一本书</button></section>}
      {state.kind === 'ready' && books.length > 0 && <>
        {books[0].lastOpenedAt && <section class="continue-card"><div><span class="eyebrow">CONTINUE READING</span><h2>{books[0].title}</h2><p>{progress(books[0])}</p></div><button onClick={() => onOpen(books[0])}>继续阅读</button></section>}
        <h2 class="section-title">全部书籍 <span>{books.length}</span></h2>
        <section class="book-grid" aria-label="书籍列表">{books.map(book => <article class="book-card" key={book.id}>
          <div class="book-cover">{book.format.toUpperCase()}</div>
          <div class="book-info"><h3>{book.title}</h3><p class="muted">{book.author || '未知作者'} · {progress(book)}</p>
            {!book.available && <p class="warning">原文件缺失，可从备份恢复或删除</p>}
            <div class="book-actions"><button onClick={() => onOpen(book)} disabled={!book.available}>阅读</button><button class="text-button" onClick={() => action(book, 'rename')}>编辑</button><button class="text-button" onClick={() => action(book, 'export')} disabled={!book.available}>导出</button><button class="text-button" onClick={() => action(book, 'archive')}>归档</button><button class="text-button danger" onClick={() => action(book, 'delete')}>删除</button></div>
          </div>
        </article>)}</section>
      </>}
    </main>
  );
}
