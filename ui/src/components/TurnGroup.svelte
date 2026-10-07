<!-- Owns one user message and its grouped agent activity and replies. -->
<script lang="ts">
  import type { TranscriptGroup } from '../lib/transcript';
  import { duration, formatTokens } from '../lib/format';
  import ToolStep from './ToolStep.svelte';
  import ApprovalCard from './ApprovalCard.svelte';
  let {
    group,
    running = false,
    cwd = '',
    onanswer,
  }: {
    group: TranscriptGroup;
    running?: boolean;
    cwd?: string;
    onanswer: (id: string, decision: 'allow' | 'deny', always: boolean) => Promise<void>;
  } = $props();
  const activity = $derived(
    group.events.filter((event) => event.t === 'tool' || event.t === 'thinking'),
  );
  const end = $derived([...group.events].reverse().find((event) => event.t === 'turn_end'));
  const start = $derived(group.events.find((event) => event.t === 'turn_start'));
</script>

<article class="turn">
  {#if group.user}
    <section class="user-message" aria-label="Your message">
      <div class="speaker">
        YOU <span
          >{['message', 'user'].includes(group.user.kind) || !group.user.kind
            ? ''
            : group.user.kind}</span
        >
      </div>
      <div class="message">{group.user.text}</div>
      {#if group.user.steered}<span class="steered">sent while agy worked</span>{/if}
      {#each group.user.attachments ?? [] as attachment (attachment.path)}<span class="attachment"
          >{attachment.name}</span
        >{/each}
    </section>
  {/if}
  {#if start || activity.length}
    <details class="activity">
      <summary
        ><span class="activity-line"></span><span
          >{running && !end ? 'Working…' : 'Agent activity'}</span
        >{#if activity.length}<span class="steps"
            >{activity.length} {activity.length === 1 ? 'step' : 'steps'}</span
          >{/if}<span class="chevron">›</span></summary
      >
      <div class="activity-body">
        {#if start?.t === 'turn_start'}<p class="routing">
            Turn {start.turn} · {start.model} · {start.slot}
          </p>{/if}
        {#each activity as event (`${event.t}-${event.turn}-${event.step}`)}
          {#if event.t === 'tool'}<ToolStep {event} />
          {:else if event.t === 'thinking'}<details class="thinking">
              <summary>Thinking <span>{formatTokens(event.tokens)} tokens</span></summary>
              <pre>{event.text}</pre>
            </details>{/if}
        {/each}
      </div>
    </details>
  {/if}
  {#each group.events as event, index (`${event.t}-${event.ts}-${index}`)}
    {#if event.t === 'assistant' || event.t === 'delta'}
      <section class="reply" aria-label="Agent reply">
        <div class="speaker">
          ANTIQUOTA {#if event.t === 'delta'}<span class="streaming">streaming</span>{/if}
        </div>
        <div class="message">{event.text}</div>
      </section>
    {:else if event.t === 'notice'}<p
        class="notice"
        class:error={event.kind === 'error'}
        role={event.kind === 'error' ? 'alert' : undefined}
      >
        {event.text}
      </p>
    {:else if event.t === 'approval'}<ApprovalCard {event} {cwd} {onanswer} />
    {:else if event.t === 'shell'}<details class="shell">
        <summary><code>{event.cmd}</code></summary>
        <pre>{event.output}</pre>
      </details>{/if}
  {/each}
  {#if end?.t === 'turn_end'}<div
      class="turn-end"
      class:error={!['completed', 'success', 'stopped'].includes(end.status.toLowerCase())}
    >
      <span class="end-dot"></span><span>{end.status}</span><span>{duration(end.secs)}</span><span
        >{formatTokens(end.usage.input_tokens)} in / {formatTokens(end.usage.output_tokens)} out</span
      >
    </div>{/if}
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
  .steered {
    display: block;
    margin-top: var(--space-2);
    color: var(--ink-4);
    font: 10px var(--mono);
  }
  .attachment {
    display: inline-block;
    margin-top: var(--space-2);
    border: 1px solid var(--rule-2);
    border-radius: var(--r-sm);
    padding: var(--space-1) var(--space-2);
    color: var(--ink-2);
    font: 10px var(--mono);
  }
  .reply {
    padding: var(--space-5) 0 var(--space-3);
  }
  .reply .speaker {
    color: var(--baton);
  }
  .reply .streaming {
    color: var(--baton);
  }
  .reply .message {
    font-size: 14px;
  }
  .activity {
    margin-top: var(--space-3);
  }
  .activity > summary {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    list-style: none;
    cursor: pointer;
    min-height: 36px;
    color: var(--ink-3);
    font-size: 11px;
  }
  summary::-webkit-details-marker {
    display: none;
  }
  .activity-line {
    width: 12px;
    height: 1px;
    background: var(--baton);
  }
  .steps {
    color: var(--ink-4);
    font: 10px var(--mono);
  }
  .chevron {
    margin-left: auto;
    color: var(--ink-4);
  }
  details[open] > summary .chevron {
    transform: rotate(90deg);
  }
  .activity-body {
    padding: var(--space-2) var(--space-3);
    border-left: 1px solid var(--rule-2);
    margin-left: 5px;
  }
  .routing {
    font: 10px var(--mono);
    color: var(--ink-4);
    padding-bottom: var(--space-3);
    overflow-wrap: anywhere;
  }
  .thinking {
    padding: var(--space-2) 0;
    font-size: 11px;
    color: var(--ink-3);
  }
  .thinking summary {
    cursor: pointer;
    min-height: 32px;
  }
  .thinking span {
    margin-left: var(--space-2);
    color: var(--ink-4);
    font-family: var(--mono);
  }
  .thinking pre {
    color: var(--ink-2);
  }
  .notice {
    border-left: 2px solid var(--rule-3);
    padding: var(--space-2) var(--space-3);
    margin-top: var(--space-3);
    color: var(--ink-2);
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .notice.error {
    border-color: var(--bad);
    color: var(--bad);
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
