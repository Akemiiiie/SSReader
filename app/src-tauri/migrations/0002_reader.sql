ALTER TABLE books ADD COLUMN txt_encoding TEXT;
ALTER TABLE books ADD COLUMN page_count INTEGER;
ALTER TABLE books ADD COLUMN paragraph_count INTEGER;

CREATE TABLE book_preferences (
    book_id TEXT PRIMARY KEY REFERENCES books(id) ON DELETE CASCADE,
    pdf_mode TEXT NOT NULL DEFAULT 'reflow' CHECK (pdf_mode IN ('reflow', 'original'))
);
