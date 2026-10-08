<!-- Renders one prompt and its chronological agent activity, replies and notices. -->
<script lang="ts">
  import { preferences } from '../lib/preferences';
  import { transcriptItems, userPresentation, type TranscriptGroup } from '../lib/transcript';
  import type { Expansion, TranscriptActions } from '../lib/transcript-actions';
  import { duration, formatTokens } from '../lib/format';
  import ActivityGroup from './ActivityGroup.svelte';
  import Thinking from './Thinking.svelte';
  import Markdown from './Markdown.svelte';
  import Notice from './Notice.svelte';
  import ApprovalCard from './ApprovalCard.svelte';
  import { asJob, jobFailed, jobStatus } from '../lib/jobs';
  import { extensionLabel } from '../lib/media';
  let {
    group,
    running = false,
    busy = false,
    cwd = '',
    onanswer,
    onlog,
    onlogin,
    attachmentUrl,
    actions,
    expand,
  }: {
    group: TranscriptGroup;
    running?: boolean;
    busy?: boolean;
    cwd?: string;
    onanswer: (id: string, decision: 'allow' | 'deny', always: boolean) => Promise<void>;
    onlog: (jid: string) => void;
    onlogin: () => void;
    attachmentUrl: (name: string) => string;
    actions: TranscriptActions;
    expand: Expansion;
  } = $props();
  const prefs = preferences.state;
  const items = $derived(transcriptItems(group.events));
  const user = $derived(group.user ? userPresentation(group.user) : null);
</script>

<article class="turn">
  {#if group.user && user}
    <section class="user-message" aria-label="Your message" title={user.tooltip}>
      <div class="speaker">{user.label}</div>
      <div class="message">{user.text}</div>
      {#if group.user.steered}<span class="steered">sent while agy worked</span>{/if}
      {#if group.user.attachments?.length}<div class="attachments">
          {#each group.user.attachments as attachment (attachment.path)}
            {#if attachment.image}<a
                class="thumb"
                href={attachmentUrl(attachment.name)}
                target="_blank"
                rel="noopener"
                title={attachment.name}
                ><img src={attachmentUrl(attachment.name)} alt={attachment.name} /></a
              >
            {:else}<span class="attachment" title={attachment.name}
                ><span class="kind">{extensionLabel(attachment.name)}</span><span class="name"
                  >{attachment.name}</span
                ></span
              >{/if}
          {/each}
        </div>{/if}
    </section>
  {/if}
  {#each items as item (item.key)}
    {#if 'events' in item}<ActivityGroup events={item.events} {cwd} {expand} />
    {:else}{@const event = item.event}
      {#if event.t === 'turn_start'}
        <div
          class="routing"
          class:live={running &&
            !group.events.some((record) => record.t === 'turn_end' && record.turn === event.turn)}
        >
          <span>Turn {event.turn}</span><span title={event.slot}>{event.slot}</span><span
            >{event.model}</span
          >
          {#if $prefs.stamps}<time datetime={new Date(event.ts * 1000).toISOString()}
              >{new Date(event.ts * 1000).toLocaleTimeString([], {
                hour: '2-digit',
                minute: '2-digit',
              })}</time
            >{/if}
          <button
            class="quiet rewind"
            aria-label={`Rewind before turn ${event.turn}`}
            disabled={busy}
            onclick={() => actions.rewind(event.turn)}>↶ rewind</button
          >
        </div>
      {:else if event.t === 'thinking'}<Thinking {event} {expand} {actions} />
      {:else if event.t === 'assistant' || event.t === 'delta'}
        <section class="reply" class:live={event.t === 'delta'} aria-label="Agent reply">
          <div class="speaker">
            ANTIQUOTA {#if event.t === 'delta'}<span class="streaming">streaming</span
              >{:else}<button
                class="quiet copy-message"
                aria-label="Copy message"
                onclick={() => actions.copy(event.text)}>Copy</button
              >{/if}
          </div>
          <Markdown text={event.text} onfile={actions.file} oncopy={actions.copy} />
        </section>
      {:else if event.t === 'notice'}<Notice {event} {actions} {onlogin} {busy} />
      {:else if event.t === 'approval'}<ApprovalCard {event} {cwd} {onanswer} />
      {:else if event.t === 'job' && asJob(event.job)}{@const job = asJob(event.job)!}
        <p class="jobline" class:bad={jobFailed(event.phase, job)}>
          <span class="mark">{event.phase === 'ended' ? '■ job' : '▶ job'}</span><code
            title={job.cmd}>{job.cmd}</code
          ><span class="detail"
            >{jobStatus(event.phase, job)}{event.phase !== 'ended' && job.by === 'agent'
              ? ' · by agy'
              : ''}</span
          ><button class="small" onclick={() => onlog(job.id)}>Log</button>
        </p>
      {:else if event.t === 'shell'}<details class="shell">
          <summary><code>{event.cmd}</code></summary>
          <pre>{event.output}</pre>
        </details>
      {:else if event.t === 'turn_end'}<div
          class="turn-end"
          class:error={!['completed', 'success', 'stopped'].includes(event.status.toLowerCase())}
        >
          {#if $prefs.stamps}<time datetime={new Date(event.ts * 1000).toISOString()}
              >{new Date(event.ts * 1000).toLocaleTimeString([], {
                hour: '2-digit',
                minute: '2-digit',
              })}</time
            >{/if}
          <span class="end-dot"></span><span>{event.status}</span><span>{duration(event.secs)}</span
          ><span
            >{formatTokens(event.usage.input_tokens)} in / {formatTokens(event.usage.output_tokens)} out</span
          ><span>{event.slot}</span>
        </div>{/if}
    {/if}
  {/each}
</article>

<style>
  .turn {
    padding-bottom: var(--space-6);
  }
  .speaker {
    font: 10px/1.5 var(--mono);
    letter-spacing: 0.06em;
    color: var(--ink-3);
    margin-bottom: var(--space-2);
  }
  .speaker span {
    margin-left: var(--space-2);
    letter-spacing: 0;
    color: var(--ink-4);
  }
  .user-message {
    background: var(--night-1);
    border: 1px solid var(--rule);
    border-radius: var(--r-md);
    padding: var(--space-4);
  }
  .user-message .message {
    font-size: 13px;
  }
  .message {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    line-height: 1.7;
  }
  .jobline {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: var(--space-2) 0;
    font: 11px var(--mono);
    color: var(--ink-3);
    min-width: 0;
  }
  .jobline .mark {
    color: var(--baton);
    flex-shrink: 0;
  }
  .jobline.bad .mark {
    color: var(--bad);
  }
  .jobline code {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-2);
  }
  .jobline .detail {
    margin-left: auto;
    flex-shrink: 0;
  }
  .jobline .small {
    font-size: 10px;
    min-height: 22px;
    padding: 0 var(--space-2);
  }
  .steered {
    display: block;
    margin-top: var(--space-2);
    color: var(--ink-4);
    font: 10px var(--mono);
  }
  .attachments {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }
  .thumb img {
    display: block;
    min-width: 36px;
    min-height: 36px;
    max-width: 220px;
    max-height: 160px;
    object-fit: contain;
    background: var(--well);
    border-radius: var(--r-md);
    outline: 1px solid var(--rule-2);
    outline-offset: -1px;
  }
  .attachment {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    max-width: 240px;
    border: 1px solid var(--rule-2);
    border-radius: var(--r-md);
    padding: 2px var(--space-2) 2px 2px;
    background: var(--well);
    color: var(--ink-2);
    font-size: 12px;
  }
  .attachment .kind {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: var(--r-sm);
    background: var(--rule);
    color: var(--ink-3);
    font: 500 9px var(--mono);
    text-transform: uppercase;
  }
  .attachment .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .reply {
    padding: var(--space-5) 0 var(--space-3);
  }
  .copy-message {
    float: right;
    min-height: 24px;
    font: 10px var(--mono);
  }
  .routing {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    align-items: center;
    color: var(--ink-4);
    font: 10px var(--mono);
    margin: var(--space-3) 0;
  }
  .routing .rewind {
    margin-left: auto;
    font: 10px var(--mono);
    min-height: 24px;
  }
  .reply .speaker {
    color: var(--baton);
  }
  .reply .streaming {
    color: var(--baton);
  }
  .reply :global(.markdown) {
    font-size: 14px;
  }
  .turn-end {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    color: var(--ink-4);
    font: 10px var(--mono);
    margin-top: var(--space-2);
  }
  .turn-end.error {
    color: var(--bad);
  }
  .end-dot {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: currentColor;
  }
  .shell {
    padding: var(--space-3);
    background: var(--well);
    margin-top: var(--space-3);
    border-radius: var(--r-sm);
  }
  .shell summary {
    cursor: pointer;
    overflow-wrap: anywhere;
  }
</style>
