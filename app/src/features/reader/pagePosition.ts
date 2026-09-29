export function codePointOffsetAtY(element: HTMLElement, boundaryY: number): number {
  const node = element.firstChild;
  if (!(node instanceof Text)) return 0;
  const points = Array.from(node.textContent || '');
  const offsets = [0];
  for (const point of points) offsets.push(offsets[offsets.length - 1] + point.length);
  const range = window.document.createRange();
  let start = 0;
  let end = points.length;
  while (start < end) {
    const middle = Math.floor((start + end) / 2);
    range.setStart(node, offsets[middle]);
    range.setEnd(node, offsets[middle + 1]);
    if (range.getBoundingClientRect().bottom > boundaryY) end = middle;
    else start = middle + 1;
  }
  return start;
}

export function codePointOffsetStartingAtY(element: HTMLElement, boundaryY: number): number {
  const node = element.firstChild;
  if (!(node instanceof Text)) return 0;
  const points = Array.from(node.textContent || '');
  const offsets = [0];
  for (const point of points) offsets.push(offsets[offsets.length - 1] + point.length);
  const range = window.document.createRange();
  let start = 0;
  let end = points.length;
  while (start < end) {
    const middle = Math.floor((start + end) / 2);
    range.setStart(node, offsets[middle]);
    range.setEnd(node, offsets[middle + 1]);
    if (range.getBoundingClientRect().top >= boundaryY) end = middle;
    else start = middle + 1;
  }
  return start;
}

export function turnWithinViewport(container: HTMLElement, contentEnd: HTMLElement | null, direction: 'previous' | 'next', lineHeight: number): boolean {
  const before = container.scrollTop;
  const step = Math.max(1, container.clientHeight - Math.min(lineHeight, container.clientHeight / 3));
  if (direction === 'previous') {
    container.scrollTop = Math.max(0, before - step);
  } else if (contentEnd) {
    const remaining = contentEnd.getBoundingClientRect().bottom - container.getBoundingClientRect().bottom;
    if (remaining <= 1) return false;
    container.scrollTop = Math.min(before + step, before + remaining);
  }
  return container.scrollTop > before + 1 || container.scrollTop < before - 1;
}

export function alignCodePoint(container: HTMLElement, element: HTMLElement, codePointOffset: number): void {
  const node = element.firstChild;
  if (!(node instanceof Text) || !node.textContent) {
    container.scrollTop += element.getBoundingClientRect().top - container.getBoundingClientRect().top;
    return;
  }
  const unitOffset = Array.from(node.textContent).slice(0, codePointOffset).join('').length;
  const start = Math.min(unitOffset, node.textContent.length - 1);
  const range = window.document.createRange();
  range.setStart(node, start);
  range.setEnd(node, start + 1);
  container.scrollTop += range.getBoundingClientRect().top - container.getBoundingClientRect().top;
}
