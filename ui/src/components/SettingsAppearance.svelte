<!-- Presents device preferences without applying CSS scale to the browser root. -->
<script lang="ts">
  import { native } from '../lib/native';
  import { zoomBy } from '../lib/native-actions';
  import { preferences, transcriptModes } from '../lib/preferences';
  const prefs = preferences.state;
</script>

<section>
  <h3>Interface size</h3>
  {#if native}<div role="group" aria-label="Interface size">
      <button aria-label="Smaller interface" onclick={() => zoomBy(-0.1)}>−</button><button
        aria-label="Actual size"
        onclick={() => zoomBy(0)}>{Math.round($prefs.zoom * 100)}%</button
      ><button aria-label="Larger interface" onclick={() => zoomBy(0.1)}>+</button>
    </div>{:else}<p>
      Use your browser’s zoom controls: ⌘/Ctrl= larger, ⌘/Ctrl− smaller, ⌘/Ctrl0 actual size.
    </p>{/if}
  <h3>Transcript</h3>
  <div role="group" aria-label="Transcript mode">
    {#each transcriptModes as [mode, label] (mode)}<button
        aria-pressed={$prefs.detail === mode}
        onclick={() => preferences.change({ detail: mode })}>{label}</button
      >{/each}
  </div>
  <p>
    Compact shows your prompts, the agent's text, one line per run of tools and the outcome. Verbose
    adds turns, automatic prompts, checks, thinking and tool previews. Full also opens every step.
  </p>
  <div role="group" aria-label="Transcript view">
    {#each [['grouped', 'Grouped steps'], ['steps', 'Every step']] as [value, label] (value)}<button
        aria-pressed={$prefs.view === value}
        onclick={() => preferences.change({ view: value as 'grouped' | 'steps' })}>{label}</button
      >{/each}
  </div>
  <p>
    In verbose mode, grouped steps fold activity between replies into one expandable line, and every
    step gives each tool call its own row.
  </p>
  <label
    ><input
      type="checkbox"
      checked={$prefs.stamps}
      onchange={(e) => preferences.change({ stamps: e.currentTarget.checked })}
    />Times on turns</label
  >
</section>

<style>
  section {
    display: grid;
    gap: var(--space-4);
  }
  h3 {
    font-size: 13px;
    font-weight: 500;
  }
  p {
    color: var(--ink-3);
    font-size: 12px;
    line-height: 1.7;
  }
  label {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    font-size: 12px;
  }
  input {
    width: auto;
  }
  button {
    font-size: 11px;
  }
  button[aria-pressed='true'] {
    color: var(--baton);
    border-color: var(--baton);
  }
  div {
    display: flex;
    gap: var(--space-2);
  }
</style>
