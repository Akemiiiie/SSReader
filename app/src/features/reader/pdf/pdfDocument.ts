import { getDocument, GlobalWorkerOptions } from 'pdfjs-dist';
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url';

// PDF.js never receives a local path. The desktop service will provide bytes for a managed book.
GlobalWorkerOptions.workerSrc = workerUrl;

export function openPdfBytes(bytes: Uint8Array) {
  return getDocument({ data: bytes });
}
