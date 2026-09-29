CREATE TABLE books (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    author TEXT,
    format TEXT NOT NULL CHECK (format IN ('txt', 'pdf')),
    relative_path TEXT NOT NULL UNIQUE,
    sha256 TEXT NOT NULL UNIQUE,
    byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
    archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
    created_at TEXT NOT NULL,
    last_opened_at TEXT
);

CREATE TABLE reading_positions (
    book_id TEXT PRIMARY KEY REFERENCES books(id) ON DELETE CASCADE,
    anchor_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE bookmarks (
    id TEXT PRIMARY KEY,
    book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    anchor_key TEXT NOT NULL,
    anchor_json TEXT NOT NULL,
    label TEXT,
    created_at TEXT NOT NULL,
    UNIQUE (book_id, anchor_key)
);

CREATE TABLE reading_settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL
);

CREATE TABLE pdf_page_text (
    book_id TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    page_no INTEGER NOT NULL CHECK (page_no > 0),
    extractor_version TEXT NOT NULL,
    quality TEXT NOT NULL,
    blocks_json TEXT NOT NULL,
    PRIMARY KEY (book_id, page_no, extractor_version)
);

CREATE INDEX idx_books_recent ON books(archived, last_opened_at DESC);
CREATE INDEX idx_bookmarks_book ON bookmarks(book_id, created_at);
