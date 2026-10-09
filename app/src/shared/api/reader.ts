import { invoke } from '@tauri-apps/api/core';
import type { Anchor, AppError, BackupInfo, BookSummary, Bookmark, ImportResult, ImportSelection, PdfBlock, PdfPageText, ReadingSettings, TxtPreview, TxtSlice } from '../contracts/book';

export function asAppError(error: unknown): AppError {
  if (typeof error === 'object' && error !== null && 'code' in error && 'message' in error) return error as AppError;
  return { code: 'UNEXPECTED', message: String(error), retryable: true };
}

export const api = {
  getAiConfig: () => invoke<{ configured: boolean }>('get_ai_config'),
  setAiApiKey: (apiKey: string) => invoke<{ configured: boolean }>('set_ai_api_key', { apiKey }),
  clearAiApiKey: () => invoke<{ configured: boolean }>('clear_ai_api_key'),
  testAiConnection: () => invoke<void>('test_ai_connection'),
  listBooks: () => invoke<BookSummary[]>('list_books'),
  getBook: (bookId: string) => invoke<BookSummary>('get_book', { bookId }),
  selectImportFile: () => invoke<ImportSelection | null>('select_import_file'),
  previewTxtImport: (fileToken: string, encoding: string) => invoke<TxtPreview>('preview_txt_import', { fileToken, encoding }),
  importBook: (fileToken: string, encoding: string | null) => invoke<ImportResult>('import_book', { fileToken, encoding }),
  cancelImport: (fileToken: string) => invoke<void>('cancel_import', { fileToken }),
  updateBook: (bookId: string, title: string, author: string | null) => invoke<BookSummary>('update_book', { bookId, title, author }),
  archiveBook: (bookId: string) => invoke<void>('archive_book', { bookId }),
  deleteBook: (bookId: string) => invoke<void>('delete_book', { bookId }),
  exportBook: (bookId: string) => invoke<boolean>('export_book', { bookId }),
  getTxtSlice: (bookId: string, paragraphIndex: number, codePointOffset = 0) => invoke<TxtSlice>('get_txt_slice', { bookId, paragraphIndex, codePointOffset }),
  getTxtSliceBefore: (bookId: string, paragraphIndex: number, codePointOffset = 0) => invoke<TxtSlice | null>('get_txt_slice_before', { bookId, paragraphIndex, codePointOffset }),
  setTxtEncoding: (bookId: string, encoding: string) => invoke<void>('set_txt_encoding', { bookId, encoding }),
  getPdfBytes: async (bookId: string) => new Uint8Array(await invoke<ArrayBuffer>('get_pdf_bytes', { bookId })),
  getPdfPageText: (bookId: string, pageNo: number) => invoke<PdfPageText | null>('get_pdf_page_text', { bookId, pageNo }),
  savePdfPageText: (bookId: string, pageNo: number, quality: PdfPageText['quality'], blocks: PdfBlock[], extractorVersion: string) => invoke<void>('save_pdf_page_text', { bookId, pageNo, quality, blocks, extractorVersion }),
  savePosition: (bookId: string, anchor: Anchor) => invoke<void>('save_position', { bookId, anchor }),
  listBookmarks: (bookId: string) => invoke<Bookmark[]>('list_bookmarks', { bookId }),
  addBookmark: (bookId: string, anchor: Anchor, label: string | null = null) => invoke<Bookmark>('add_bookmark', { bookId, anchor, label }),
  renameBookmark: (id: string, label: string) => invoke<void>('rename_bookmark', { id, label }),
  deleteBookmark: (id: string) => invoke<void>('delete_bookmark', { id }),
  getReadingSettings: () => invoke<ReadingSettings>('get_reading_settings'),
  setReadingSettings: (settings: ReadingSettings) => invoke<ReadingSettings>('set_reading_settings', { settings }),
  getPdfMode: (bookId: string) => invoke<'original' | 'reflow'>('get_pdf_mode', { bookId }),
  setPdfMode: (bookId: string, mode: 'original' | 'reflow') => invoke<void>('set_pdf_mode', { bookId, mode }),
  createBackup: () => invoke<BackupInfo | null>('create_backup'),
  selectBackupFile: () => invoke<BackupInfo | null>('select_backup_file'),
  restoreBackup: () => invoke<BackupInfo>('restore_backup'),
  getShortcutStatus: () => invoke<string | null>('get_shortcut_status'),
  toggleMainWindow: () => invoke<void>('toggle_main_window'),
};
