<!-- Presents typed engine notices and safe, session-owned denial actions. -->
<script lang="ts">
  import { noticeView, type NoticeEvent } from '../lib/notices';
  import type { TranscriptActions } from '../lib/transcript-actions';
  let {
    event,
    actions,
    onlogin,
    busy,
  }: { event: NoticeEvent; actions: TranscriptActions; onlogin: () => void; busy: boolean } =
    $props();
  const view = $derived(noticeView(event));
</script>

<section
  class="notice"
  class:good={view.tone === 'ok'}
  class:bad={view.tone === 'bad'}
  class:warning={view.tone === 'warning'}
  class:handoff={event.kind === 'handoff'}
  aria-label={view.title || 'Notice'}
  role={event.kind === 'error' ? 'alert' : undefined}
>
  {#if view.title}<strong>{view.title}</strong>{/if}
  {#if event.kind === 'error'}<pre>{event.text}</pre>{:else}<p>{event.text}</p>{/if}
  {#if view.missing.length}<ul>
      {#each view.missing as item, index (index)}<li>{item}</li>{/each}
    </ul>{/if}
  {#if view.identity}<small>{view.identity}</small>{/if}
  {#if view.output}<details>
      <summary>Output</summary>
      <pre>{view.output}</pre>
    </details>{/if}
  {#if view.files.length}<ul>
      {#each view.files as file (file.path)}<li>
          <code>{file.path}</code>{#if file.outside}
            &nbsp;(it had changed after the agent's last edit){/if}
        </li>{/each}
    </ul>{/if}
  {#if view.commands.length}<ul>
      {#each view.commands as command, index (index)}<li><code>{command}</code></li>{/each}
    </ul>{/if}
  {#if event.kind === 'noaccount'}<button class="small" onclick={onlogin}>Add Google account</button
    >{/if}
  {#if event.kind === 'denied'}
    {#if !view.commands.length && view.actions.length}<p>
        Denied: {view.actions.map((action) => action.display_name || action.action).join(', ')}
      </p>{/if}
    <div class="actions">
      {#if view.rules.length}<button
          class="small"
          disabled={busy}
          onclick={() => actions.allow(view.rules, false)}>Allow {view.rules.join(', ')}</button
        ><button
          class="small primary"
          disabled={busy}
          onclick={() => actions.allow(view.rules, true)}>Allow &amp; retry</button
        >
      {:else if view.canAcceptEdits}<button
          class="small"
          disabled={busy}
          onclick={() => actions.edits()}>Switch to accept edits &amp; retry</button
        >
      {:else}<small
          >Review permissions in Settings. Arbitrary program runners require approval for each call.</small
        >{/if}
    </div>
    {#if view.rules.length}<small>Existing deny rules still apply.</small>{/if}
  {/if}
</section>

<style>
  .notice {
    border-left: 2px solid var(--rule-3);
    padding: var(--space-2) var(--space-3);
    margin-top: var(--space-3);
    color: var(--ink-2);
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  strong {
    display: block;
    color: var(--ink-3);
    font: 11px var(--mono);
    margin-bottom: var(--space-1);
  }
  p {
    margin: 0;
    white-space: pre-wrap;
  }
  .good {
    border-color: var(--ok);
  }
  .good strong {
    color: var(--ok);
  }
  .bad {
    border-color: var(--bad);
  }
  .bad strong {
    color: var(--bad);
  }
  .warning,
  .handoff {
    border-color: var(--baton);
  }
  .warning strong,
  .handoff strong {
    color: var(--baton);
  }
  small {
    display: block;
    color: var(--ink-4);
    font: 10px var(--mono);
    margin-top: var(--space-2);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }
  summary {
    cursor: pointer;
    min-height: 28px;
    padding-top: var(--space-2);
  }
  pre {
    white-space: pre-wrap;
    font-size: 11px;
    margin: var(--space-2) 0;
  }
  ul {
    margin: var(--space-2) 0;
    padding-left: 20px;
  }
</style>
