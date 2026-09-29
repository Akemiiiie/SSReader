use crate::{
    services::{backup, books, reading},
    storage::Database,
};
use std::{
    fs,
    sync::{atomic::AtomicBool, Arc},
};

#[test]
fn import_position_bookmark_backup_and_restore() {
    let root = tempfile::tempdir().unwrap();
    let data_dir = root.path().join("app-data");
    let source = root.path().join("story.txt");
    fs::write(&source, "First paragraph\n第二段 😀\nChapter 2\nEnd").unwrap();
    let db = Database::initialize(data_dir.clone()).unwrap();
    let flag = Arc::new(AtomicBool::new(false));
    let first = books::import_book(&db, &source, &flag).unwrap();
    assert!(!first.duplicate);
    let duplicate = books::import_book(&db, &source, &flag).unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(db.list_books().unwrap().len(), 1);
    let slice = reading::txt_slice(&db, &first.book.id, 1, 0).unwrap();
    assert_eq!(slice.chunks[0].text, "第二段 😀");
    let anchor = serde_json::json!({"kind":"txt","version":1,"paragraphIndex":1,"codePointOffset":2,"paragraphFingerprint":slice.chunks[0].fingerprint});
    reading::save_position(&db, &first.book.id, anchor.clone()).unwrap();
    reading::add_bookmark(&db, &first.book.id, anchor.clone(), Some("重点".into())).unwrap();
    assert_eq!(
        reading::add_bookmark(&db, &first.book.id, anchor.clone(), None)
            .unwrap_err()
            .code,
        "DUPLICATE_BOOKMARK"
    );

    let archive = root.path().join("reader.ssrb");
    let info = backup::create_backup(&db, &archive).unwrap();
    assert_eq!(info.book_count, 1);
    assert_eq!(backup::inspect_backup(&archive).unwrap().book_count, 1);
    books::delete_book(&db, &first.book.id).unwrap();
    assert!(db.list_books().unwrap().is_empty());
    backup::restore_backup(&data_dir, &archive).unwrap();
    let restored = Database::initialize(data_dir).unwrap();
    assert_eq!(restored.list_books().unwrap().len(), 1);
    assert_eq!(restored.list_books().unwrap()[0].position, Some(anchor));
    assert_eq!(
        reading::list_bookmarks(&restored, &first.book.id)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn reverse_txt_slice_reaches_content_before_a_restored_anchor() {
    let root = tempfile::tempdir().unwrap();
    let db = Database::initialize(root.path().join("app-data")).unwrap();
    let source = root.path().join("long.txt");
    fs::write(&source, format!("{}\n\n结尾", "文".repeat(12000))).unwrap();
    let imported = books::import_book(&db, &source, &Arc::new(AtomicBool::new(false))).unwrap();
    let previous = reading::txt_slice_before(&db, &imported.book.id, 0, 9000)
        .unwrap()
        .unwrap();
    assert_eq!(previous.chunks.first().unwrap().code_point_offset, 1000);
    assert_eq!(previous.next_paragraph_index, Some(0));
    assert_eq!(previous.next_code_point_offset, Some(9000));
    assert_eq!(
        previous
            .chunks
            .iter()
            .map(|chunk| chunk.text.chars().count())
            .sum::<usize>(),
        8000
    );
    let before_end = reading::txt_slice_before(&db, &imported.book.id, 2, 0)
        .unwrap()
        .unwrap();
    assert_eq!(before_end.chunks.last().unwrap().paragraph_index, 1);
    assert!(reading::txt_slice_before(&db, &imported.book.id, 0, 0)
        .unwrap()
        .is_none());
}

#[test]
fn bad_backup_does_not_replace_library() {
    let root = tempfile::tempdir().unwrap();
    let data_dir = root.path().join("app-data");
    let db = Database::initialize(data_dir.clone()).unwrap();
    let source = root.path().join("sample.txt");
    fs::write(&source, "Kept book").unwrap();
    books::import_book(&db, &source, &Arc::new(AtomicBool::new(false))).unwrap();
    let bad = root.path().join("bad.ssrb");
    fs::write(&bad, "not a backup").unwrap();
    assert_eq!(
        backup::restore_backup(&data_dir, &bad).unwrap_err().code,
        "BACKUP_CORRUPT"
    );
    assert_eq!(db.list_books().unwrap().len(), 1);
}

#[test]
fn inspection_rejects_modified_backup_content() {
    use std::io::{Read, Write};
    use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

    let root = tempfile::tempdir().unwrap();
    let data_dir = root.path().join("app-data");
    let db = Database::initialize(data_dir.clone()).unwrap();
    let source = root.path().join("sample.txt");
    fs::write(&source, "Original book").unwrap();
    books::import_book(&db, &source, &Arc::new(AtomicBool::new(false))).unwrap();
    let good = root.path().join("good.ssrb");
    backup::create_backup(&db, &good).unwrap();

    let mut original = ZipArchive::new(fs::File::open(&good).unwrap()).unwrap();
    let bad = root.path().join("modified.ssrb");
    let mut writer = ZipWriter::new(fs::File::create(&bad).unwrap());
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for index in 0..original.len() {
        let mut entry = original.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if name.starts_with("books/") {
            bytes[0] ^= 1;
        }
        writer.start_file(name, options).unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap();

    assert_eq!(
        backup::inspect_backup(&bad).unwrap_err().code,
        "BACKUP_CORRUPT"
    );
    assert_eq!(
        backup::restore_backup(&data_dir, &bad).unwrap_err().code,
        "BACKUP_CORRUPT"
    );
    assert_eq!(db.list_books().unwrap().len(), 1);
}

#[test]
fn interrupted_restore_rolls_back_on_startup() {
    let root = tempfile::tempdir().unwrap();
    let data_dir = root.path().join("app-data");
    let db = Database::initialize(data_dir.clone()).unwrap();
    let source = root.path().join("sample.txt");
    fs::write(&source, "Keep this book").unwrap();
    let imported = books::import_book(&db, &source, &Arc::new(AtomicBool::new(false))).unwrap();

    let rollback = data_dir.join("restore-rollback");
    fs::create_dir(&rollback).unwrap();
    fs::rename(
        data_dir.join("reader.sqlite3"),
        rollback.join("reader.sqlite3"),
    )
    .unwrap();
    fs::rename(data_dir.join("books"), rollback.join("books")).unwrap();
    fs::write(data_dir.join("restore-in-progress"), b"rollback").unwrap();

    let recovered = Database::initialize(data_dir.clone()).unwrap();
    assert_eq!(recovered.list_books().unwrap()[0].id, imported.book.id);
    assert!(!rollback.exists());
    assert!(!data_dir.join("restore-in-progress").exists());
}

#[test]
fn utf16_and_gb18030_samples_decode() {
    let utf16 = [0xff, 0xfe, b'H', 0, b'i', 0];
    assert_eq!(books::decode_txt(&utf16, None).unwrap().0, "Hi");
    let (encoded, _, errors) = encoding_rs::GB18030.encode("中文");
    assert!(!errors);
    assert_eq!(books::decode_txt(&encoded, None).unwrap().0, "中文");
}

#[test]
fn uncertain_txt_encoding_can_be_previewed_and_chosen() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("chinese.txt");
    let (bytes, _, errors) = encoding_rs::GB18030.encode("中文段落");
    assert!(!errors);
    fs::write(&source, bytes.as_ref()).unwrap();
    let detected = books::preview_txt(&source, None).unwrap();
    assert_eq!(detected.encoding, "gb18030");
    assert!(detected.uncertain);
    assert_eq!(detected.text, "中文段落");
    assert_eq!(
        books::preview_txt(&source, Some("gb18030")).unwrap().text,
        "中文段落"
    );
    assert_eq!(
        books::preview_txt(&source, Some("utf-8"))
            .err()
            .unwrap()
            .code,
        "ENCODING_UNKNOWN"
    );

    let db = Database::initialize(root.path().join("app-data")).unwrap();
    let imported = books::import_book_with_encoding(
        &db,
        &source,
        &Arc::new(AtomicBool::new(false)),
        Some("gb18030"),
    )
    .unwrap();
    assert_eq!(imported.book.txt_encoding.as_deref(), Some("gb18030"));
}

#[test]
fn invalid_pdf_and_oversized_txt_leave_library_untouched() {
    let root = tempfile::tempdir().unwrap();
    let db = Database::initialize(root.path().join("app-data")).unwrap();
    let flag = Arc::new(AtomicBool::new(false));
    let damaged = root.path().join("broken.pdf");
    fs::write(&damaged, b"%PDF-not-a-document").unwrap();
    assert_eq!(
        books::import_book(&db, &damaged, &flag).err().unwrap().code,
        "PDF_DAMAGED"
    );

    let oversized = root.path().join("large.txt");
    let file = fs::File::create(&oversized).unwrap();
    file.set_len(32 * 1024 * 1024 + 1).unwrap();
    assert_eq!(
        books::import_book(&db, &oversized, &flag)
            .err()
            .unwrap()
            .code,
        "FILE_TOO_LARGE"
    );
    assert!(db.list_books().unwrap().is_empty());
}

#[test]
fn backup_restores_into_clean_data_directory() {
    let root = tempfile::tempdir().unwrap();
    let source_dir = root.path().join("first-user");
    let target_dir = root.path().join("second-user");
    let db = Database::initialize(source_dir).unwrap();
    let source = root.path().join("story.txt");
    fs::write(&source, "Chapter 1\nFirst line\n第二段 😀").unwrap();
    let imported = books::import_book(&db, &source, &Arc::new(AtomicBool::new(false))).unwrap();
    let fingerprint = reading::txt_slice(&db, &imported.book.id, 2, 0)
        .unwrap()
        .chunks[0]
        .fingerprint
        .clone();
    let anchor = serde_json::json!({"kind":"txt","version":1,"paragraphIndex":2,"codePointOffset":3,"paragraphFingerprint":fingerprint});
    reading::save_position(&db, &imported.book.id, anchor.clone()).unwrap();
    reading::add_bookmark(&db, &imported.book.id, anchor.clone(), None).unwrap();
    let mut settings = reading::get_settings(&db).unwrap();
    settings.font_size = 23;
    reading::set_settings(&db, settings).unwrap();
    let archive = root.path().join("transfer.ssrb");
    backup::create_backup(&db, &archive).unwrap();

    Database::initialize(target_dir.clone()).unwrap();
    backup::restore_backup(&target_dir, &archive).unwrap();
    let restored = Database::initialize(target_dir).unwrap();
    assert_eq!(restored.list_books().unwrap()[0].position, Some(anchor));
    assert_eq!(
        reading::list_bookmarks(&restored, &imported.book.id)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(reading::get_settings(&restored).unwrap().font_size, 23);
    assert_eq!(
        reading::txt_slice(&restored, &imported.book.id, 2, 0)
            .unwrap()
            .chunks[0]
            .text,
        "第二段 😀"
    );
}
