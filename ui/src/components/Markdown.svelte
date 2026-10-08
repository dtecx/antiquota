<!-- Renders the escaped transcript grammar and delegates its generated links and copy buttons. -->
<script lang="ts">
  import { markdown } from '../lib/markdown';
  let {
    text,
    onfile,
    oncopy,
  }: {
    text: string;
    onfile: (path: string) => Promise<void>;
    oncopy: (text: string) => Promise<void>;
  } = $props();
  function attach(node: HTMLElement) {
    function click(event: MouseEvent) {
      if (!(event.target instanceof Element)) return;
      const link = event.target.closest<HTMLAnchorElement>('a[data-file]');
      if (link && node.contains(link)) {
        event.preventDefault();
        void onfile(link.dataset.file ?? '');
        return;
      }
      const button = event.target.closest<HTMLButtonElement>('button.copy-code');
      if (button && node.contains(button))
        void oncopy(button.closest('pre')?.querySelector('code')?.textContent ?? '');
    }
    node.addEventListener('click', click);
    return { destroy: () => node.removeEventListener('click', click) };
  }
</script>

<!-- eslint-disable-next-line svelte/no-at-html-tags -->
<div class="markdown" use:attach>{@html markdown(text)}</div>

<style>
  .markdown {
    line-height: 1.7;
    overflow-wrap: anywhere;
  }
  .markdown :global(p) {
    margin: 0 0 var(--space-3);
  }
  .markdown :global(p:last-child) {
    margin-bottom: 0;
  }
  .markdown :global(h3),
  .markdown :global(h4),
  .markdown :global(h5),
  .markdown :global(h6) {
    margin: var(--space-4) 0 var(--space-2);
    line-height: 1.4;
    font-weight: 600;
  }
  .markdown :global(h3) {
    font-size: 19px;
  }
  .markdown :global(h4) {
    font-size: 17px;
  }
  .markdown :global(h5) {
    font-size: 15px;
  }
  .markdown :global(h6) {
    font-size: 14px;
  }
  .markdown :global(ul),
  .markdown :global(ol) {
    padding-left: 24px;
    margin: var(--space-2) 0;
  }
  .markdown :global(blockquote) {
    margin: var(--space-3) 0;
    padding: var(--space-2) var(--space-3);
    border-left: 2px solid var(--rule-3);
    color: var(--ink-3);
  }
  .markdown :global(code) {
    font: 12px var(--mono);
    background: var(--well);
    border-radius: var(--r-sm);
    padding: 1px 4px;
  }
  .markdown :global(pre.code) {
    position: relative;
    padding: var(--space-4);
    margin: var(--space-3) 0;
    background: var(--well);
    border: 1px solid var(--rule);
    border-radius: var(--r-sm);
    overflow-x: auto;
    white-space: pre;
  }
  .markdown :global(pre.code code) {
    display: block;
    padding: 0;
    background: transparent;
  }
  .markdown :global(.copy-code) {
    position: sticky;
    float: right;
    right: 0;
    font: 10px var(--mono);
    min-height: 24px;
    margin-top: var(--space-2);
  }
  .markdown :global(table) {
    display: block;
    overflow-x: auto;
    border-collapse: collapse;
    margin: var(--space-3) 0;
    font-size: 12px;
  }
  .markdown :global(th),
  .markdown :global(td) {
    border: 1px solid var(--rule);
    padding: var(--space-2) var(--space-3);
    text-align: left;
  }
  .markdown :global(th) {
    background: var(--well);
  }
  .markdown :global(a) {
    color: var(--baton);
  }
</style>
