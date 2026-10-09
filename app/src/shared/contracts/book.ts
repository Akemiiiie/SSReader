export interface BookSummary {
  id: string;
  title: string;
  author: string | null;
  format: 'txt' | 'pdf';
  available: boolean;
  lastOpenedAt: string | null;
  pageCount: number | null;
  paragraphCount: number | null;
  txtEncoding: string | null;
  position: Anchor | null;
}

export type Anchor =
  | { kind: 'txt'; version: 1; paragraphIndex: number; codePointOffset: number; paragraphFingerprint: string }
  | { kind: 'pdf'; version: 1; pageNo: number; mode: 'original' | 'reflow'; blockId?: string; codePointOffset?: number; scrollRatio?: number; textFingerprint?: string };

export interface ImportSelection { token: string; fileName: string; format: 'txt' | 'pdf'; byteSize: number; txtEncoding: string | null; preview: string | null; encodingUncertain: boolean }
export interface TxtPreview { encoding: string; text: string; uncertain: boolean }
export interface ImportResult { book: BookSummary; duplicate: boolean }

export interface TxtChunk { paragraphIndex: number; codePointOffset: number; text: string; fingerprint: string }
export interface Chapter { title: string; paragraphIndex: number }
export interface TxtSlice {
  chunks: TxtChunk[];
  totalParagraphs: number;
  nextParagraphIndex: number | null;
  nextCodePointOffset: number | null;
  encoding: string;
  chapters: Chapter[];
}

export interface Bookmark { id: string; anchor: Anchor; label: string | null; createdAt: string }

export interface ReadingSettings {
  fontSize: number;
  fontFamily?: 'book' | 'system' | 'mono';
  fontColor: string | null;
  lineHeight: number;
  theme: 'paper' | 'light' | 'dark';
  sidebarCollapsed: boolean;
  pdfZoom: number;
  alwaysOnTop: boolean;
  opacity: number;
  previousPageShortcut: string;
  nextPageShortcut: string;
  windowShortcutMode: 'hold' | 'double';
}

export interface PdfBlock { id: string; text: string; x: number; y: number; width: number; height: number }
export interface PdfPageText { pageNo: number; quality: 'good' | 'uncertain' | 'no-text'; blocks: PdfBlock[]; extractorVersion: string }
export interface BackupInfo { bookCount: number; byteSize: number; schemaVersion: number }

export interface SelectionSnapshot { bookId: string; format: 'txt' | 'pdf'; text: string; anchor: Anchor; sourceLabel: string; sourceSentence?: string; contextBefore?: string; contextAfter?: string }

export interface AppError {
  code: string;
  message: string;
  retryable: boolean;
  details?: string;
}
