"""Generate small local-only files for the milestone 1 desktop smoke test."""

from pathlib import Path


ROOT = Path(__file__).parent


def make_pdf(path: Path, pages: list[bytes], rotations: list[int] | None = None) -> None:
    objects: list[bytes] = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [" + b" ".join(f"{4 + i * 2} 0 R".encode() for i in range(len(pages))) + b"] /Count " + str(len(pages)).encode() + b" >>",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    ]
    for i, content in enumerate(pages):
        page_no = 4 + i * 2
        rotation = f" /Rotate {rotations[i]}" if rotations else ""
        objects.append(f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792]{rotation} /Resources << /Font << /F1 3 0 R >> >> /Contents {page_no + 1} 0 R >>".encode())
        objects.append(b"<< /Length " + str(len(content)).encode() + b" >>\nstream\n" + content + b"\nendstream")

    output = bytearray(b"%PDF-1.4\n")
    offsets = [0]
    for index, body in enumerate(objects, 1):
        offsets.append(len(output))
        output.extend(f"{index} 0 obj\n".encode() + body + b"\nendobj\n")
    start = len(output)
    output.extend(f"xref\n0 {len(objects) + 1}\n0000000000 65535 f \n".encode())
    for offset in offsets[1:]:
        output.extend(f"{offset:010} 00000 n \n".encode())
    output.extend(f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n".encode())
    path.write_bytes(output)


ROOT.joinpath("utf8.txt").write_text("Chapter 1\nA quiet start.\n第二段 😀\nChapter 2\nThe end.", encoding="utf-8")
ROOT.joinpath("utf16.txt").write_text("UTF-16 sample\n第二段 😀", encoding="utf-16")
ROOT.joinpath("gb18030.txt").write_bytes("GB18030 sample\n中文段落".encode("gb18030"))
text = b"BT /F1 24 Tf 72 700 Td (Hello PDF world) Tj ET"
scan = b"q 0.8 g 100 100 400 500 re f Q"
make_pdf(ROOT / "text.pdf", [text])
make_pdf(ROOT / "scan.pdf", [scan])
make_pdf(ROOT / "mixed.pdf", [text, scan])
make_pdf(ROOT / "columns.pdf", [b"BT /F1 16 Tf 50 700 Td (Left column first) Tj 300 0 Td (Right column second) Tj ET"])
make_pdf(ROOT / "rotated.pdf", [text], [90])
