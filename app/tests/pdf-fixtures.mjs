import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { getDocument } from 'pdfjs-dist/legacy/build/pdf.mjs';

for (const [name, pages, rotation, withText] of [
  ['text', 1, 0, [true]],
  ['scan', 1, 0, [false]],
  ['mixed', 2, 0, [true, false]],
  ['columns', 1, 0, [true]],
  ['rotated', 1, 90, [true]],
]) {
  const data = new Uint8Array(readFileSync(new URL(`./fixtures/${name}.pdf`, import.meta.url)));
  const standardFontDataUrl = fileURLToPath(new URL('../node_modules/pdfjs-dist/standard_fonts/', import.meta.url)).replaceAll('\\', '/');
  const pdf = await getDocument({ data, standardFontDataUrl }).promise;
  assert.equal(pdf.numPages, pages, `${name}: page count`);
  for (let pageNo = 1; pageNo <= pages; pageNo++) {
    const page = await pdf.getPage(pageNo);
    assert.equal(page.rotate, rotation, `${name}: rotation`);
    const text = await page.getTextContent();
    assert.equal(text.items.some(item => 'str' in item && item.str.trim()), withText[pageNo - 1], `${name}: text layer on page ${pageNo}`);
  }
  await pdf.destroy();
  console.log(`${name}: ${pages} page(s), rotation ${rotation}°, text ${withText.join('/')}`);
}
