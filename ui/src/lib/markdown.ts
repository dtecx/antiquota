/** Renders only the escaped, bounded Markdown grammar supported by the transcript. */
export function escapeMarkup(text: string): string {
  return text.replace(
    /[&<>"']/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!,
  );
}

const placeholder = new RegExp(`${String.fromCharCode(0)}(\\d+)${String.fromCharCode(0)}`, 'g');

function inline(text: string): string {
  const saved: string[] = [];
  const hold = (html: string) => `\u0000${saved.push(html) - 1}\u0000`;
  let result = text.replace(/`([^`\n]+)`/g, (_, code: string) =>
    hold(`<code>${escapeMarkup(code)}</code>`),
  );
  result = escapeMarkup(result)
    .replace(/\[([^\]\n]+)\]\(([^)\s]+)\)/g, (_, label: string, url: string) => {
      label = label.replace(placeholder, (_, index: string) => saved[Number(index)] ?? '');
      if (url.includes('\u0000')) return label;
      if (/^https?:\/\//i.test(url))
        return hold(`<a href="${url}" target="_blank" rel="noopener noreferrer">${label}</a>`);
      if (/^file:\/\//i.test(url))
        return hold(
          `<a href="#" class="path" data-file="${url.slice(7)}" title="Open ${url.slice(7)}">${label}</a>`,
        );
      return label;
    })
    .replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>')
    .replace(/(^|[^*\w])\*([^*\n]+)\*(?!\w)/g, '$1<em>$2</em>')
    .replace(/\n/g, '<br>');
  return result.replace(placeholder, (_, index: string) => saved[Number(index)] ?? '');
}

export function markdown(source: string): string {
  const lines = source.replace(/\r/g, '').replaceAll(String.fromCharCode(0), '\ufffd').split('\n');
  const block = /^(```|#{1,4}\s|>|\s*([-*+]|\d+\.)\s|\|)/;
  const cells = (line: string) =>
    line
      .trim()
      .replace(/^\||\|$/g, '')
      .split('|')
      .map((cell) => cell.trim());
  const output: string[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i]!;
    if (/^```/.test(line)) {
      const code: string[] = [];
      i++;
      while (i < lines.length && !/^```/.test(lines[i]!)) code.push(lines[i++]!);
      if (i < lines.length) i++;
      output.push(
        `<pre class="code"><code>${escapeMarkup(code.join('\n'))}</code><button type="button" class="copy-code" aria-label="Copy code">Copy</button></pre>`,
      );
    } else if (/^#{1,4}\s/.test(line)) {
      const level = line.match(/^#+/)![0].length + 2;
      output.push(`<h${level}>${inline(line.replace(/^#+\s*/, ''))}</h${level}>`);
      i++;
    } else if (/^\s*([-*+]|\d+\.)\s+/.test(line)) {
      const ordered = /^\s*\d+\./.test(line),
        tag = ordered ? 'ol' : 'ul';
      const items: string[] = [];
      const pattern = ordered ? /^\s*\d+\.\s+/ : /^\s*[-*+]\s+/;
      while (i < lines.length && pattern.test(lines[i]!))
        items.push(lines[i++]!.replace(pattern, ''));
      output.push(`<${tag}>${items.map((item) => `<li>${inline(item)}</li>`).join('')}</${tag}>`);
    } else if (/^>/.test(line)) {
      const quote: string[] = [];
      while (i < lines.length && /^>/.test(lines[i]!)) quote.push(lines[i++]!.replace(/^>\s?/, ''));
      output.push(`<blockquote>${inline(quote.join('\n'))}</blockquote>`);
    } else if (/^\|.*\|\s*$/.test(line) && /^\|[\s:|-]+\|\s*$/.test(lines[i + 1] ?? '')) {
      const head = cells(line),
        rows: string[][] = [];
      i += 2;
      while (i < lines.length && /^\|.*\|\s*$/.test(lines[i]!)) rows.push(cells(lines[i++]!));
      output.push(
        `<table><thead><tr>${head.map((cell) => `<th>${inline(cell)}</th>`).join('')}</tr></thead><tbody>${rows.map((row) => `<tr>${row.map((cell) => `<td>${inline(cell)}</td>`).join('')}</tr>`).join('')}</tbody></table>`,
      );
    } else if (!line.trim()) i++;
    else {
      const paragraph: string[] = [];
      while (i < lines.length && lines[i]!.trim() && !block.test(lines[i]!))
        paragraph.push(lines[i++]!);
      if (!paragraph.length) paragraph.push(lines[i++]!);
      output.push(`<p>${inline(paragraph.join('\n'))}</p>`);
    }
  }
  return output.join('');
}

export function fileTarget(text: string): { path: string; line?: number } {
  const split = text.lastIndexOf('#'),
    hash = split >= 0 ? text.slice(split + 1) : '';
  const match = /^(?:L|line-)?([1-9]\d*)$/i.exec(hash);
  let path = match ? text.slice(0, split) : text;
  try {
    path = decodeURIComponent(path);
  } catch {
    /* Preserve malformed paths for the engine's validation. */
  }
  const line = match ? Number(match[1]) : 0;
  return { path, ...(Number.isSafeInteger(line) && line > 0 ? { line } : {}) };
}
