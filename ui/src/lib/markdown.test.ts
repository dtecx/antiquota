/** Proves model text stays inert while the supported grammar remains useful. */
import { expect, it } from 'vitest';
import { fileTarget, markdown } from './markdown';

it('escapes markup, quote attributes, scripts and placeholder characters', () => {
  const html = markdown(
    '<script>window.hacked=true</script> <img src=x onerror="alert(1)"> \u00000\u0000',
  );
  expect(html).not.toContain('<script>');
  expect(html).not.toContain('<img');
  expect(html).toContain('&lt;script&gt;');
  expect(html).toContain('&quot;');
  expect(html).toContain('\ufffd0\ufffd');
});
it('renders four heading levels, mixed lists, quotes, tables and inline formatting', () => {
  const html = markdown(
    '# One\n## Two\n### Three\n#### Four\n\n- item\n+ another\n1. ordered\n2. second\n> quoted\n> line\n\n| Head | Two |\n| --- | :--: |\n| cell | **bold** |\n\nA *word* and `code<>&`\nnext.',
  );
  for (const tag of [
    'h3',
    'h4',
    'h5',
    'h6',
    'ul',
    'ol',
    'blockquote',
    'table',
    'thead',
    'tbody',
    'strong',
    'em',
  ])
    expect(html).toContain(`<${tag}>`);
  expect(html).toContain('<code>code&lt;&gt;&amp;</code>');
  expect(html).toContain('<br>next.');
});
it('keeps fenced content literal and handles empty and unterminated fences', () => {
  expect(markdown('```ts\n**bold** <script>\n```')).toContain(
    '<code>**bold** &lt;script&gt;</code>',
  );
  expect(markdown('```\n```')).toContain('<code></code>');
  expect(markdown('```\nlast')).toContain('<code>last</code>');
  expect(markdown('')).toBe('');
});
it('permits only external HTTP links and file-viewer links with escaped attributes', () => {
  const html = markdown(
    '[web](https://example.test/?x="bad") [file](file:///project/a%20b.md#L2) [bad](javascript:alert) [bad2](data:text/html)',
  );
  expect(html).toContain(
    'href="https://example.test/?x=&quot;bad&quot;" target="_blank" rel="noopener noreferrer"',
  );
  expect(html).toContain('data-file="/project/a%20b.md#L2"');
  expect(html).not.toContain('href="javascript:');
  expect(html).not.toContain('href="data:');
  expect(markdown('`[web](https://example.test)`')).not.toContain('<a');
});
it('decodes file paths and accepts only positive safe line fragments', () => {
  expect(fileTarget('/project/a%20b.md#L2')).toEqual({ path: '/project/a b.md', line: 2 });
  expect(fileTarget('/project/a.md#line-3')).toEqual({ path: '/project/a.md', line: 3 });
  expect(fileTarget('/project/a.md#L0')).toEqual({ path: '/project/a.md#L0' });
  expect(fileTarget('/bad%path')).toEqual({ path: '/bad%path' });
  expect(fileTarget('/a#9999999999999999999999')).toEqual({ path: '/a' });
});

it('keeps inline code inside link labels bounded and refuses code placeholders in URLs', () => {
  expect(markdown('[`safe <code>`](https://example.test)')).toContain(
    '<code>safe &lt;code&gt;</code></a>',
  );
  expect(markdown('[label](https://example.test/`path`)')).not.toContain('<a');
});
