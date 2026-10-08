<!-- Uses existing field tokens for new-session defaults and goal controls. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { native, autostartEnabled, setAutostart } from '../lib/native';
  import type { SettingsStore, SettingsSection } from '../lib/settings';
  import type { Model } from '../gen/Model';
  import type { Settings } from '../gen/Settings';
  import { modelFamilies, pickFamily } from '../lib/models';
  import { preferences, notificationLabels, type NotificationKind } from '../lib/preferences';
  let {
    settings,
    section,
    models,
  }: { settings: SettingsStore; section: SettingsSection; models: Model[] } = $props();
  const view = $derived(settings.state);
  const prefs = preferences.state;
  let alive = true;
  let autostartAvailable = $state(false);
  let autostart = $state(false),
    nativeBusy = $state(false);
  onMount(() => {
    if (native)
      void autostartEnabled()
        .then((value) => {
          if (alive) {
            autostart = value;
            autostartAvailable = true;
          }
        })
        .catch(() => {
          if (alive) autostartAvailable = false;
        });
    return () => {
      alive = false;
    };
  });
  async function changeAutostart(on: boolean) {
    if (nativeBusy) return;
    nativeBusy = true;
    try {
      await setAutostart(on);
      const enabled = await autostartEnabled();
      if (alive) autostart = enabled;
    } catch (error) {
      if (alive) settings.fail(error);
    } finally {
      nativeBusy = false;
    }
  }
  const families = $derived(modelFamilies(models, $view.fields?.model ?? 'gemini-3.8-flash-high'));
  const family = $derived(
    families.find((f) => f.variants.some((v) => v.id === $view.fields?.model)) ?? families[0],
  );
  function change(key: keyof Settings, value: string) {
    settings.field(key, value);
    void settings.saveField(key);
  }
  const numbers: { key: keyof Settings; label: string; min: number; max: number }[] = [
    { key: 'max_rounds', label: 'Rounds', min: 1, max: 500 },
    { key: 'approval_timeout', label: 'Approval wait', min: 1, max: 240 },
    { key: 'auto_continue', label: 'Retries after a denial', min: 0, max: 10 },
  ];
</script>

{#if $view.fields}
  <section class="defaults" aria-label={section === 'agents' ? 'Agents and goals' : 'General'}>
    {#if section === 'general'}
      <h3>New sessions</h3>
      <div class="fields">
        <label
          >Default model family<select
            aria-label="Default model family"
            value={family?.name}
            onchange={(e) =>
              change('model', pickFamily(models, $view.fields!.model, e.currentTarget.value))}
            >{#each families as f (f.name)}<option>{f.name}</option>{/each}</select
          ></label
        >
        <label
          >Default effort<select
            aria-label="Default effort"
            value={$view.fields.model}
            disabled={(family?.variants.length ?? 0) < 2}
            onchange={(e) => change('model', e.currentTarget.value)}
            >{#each family?.variants ?? [] as variant (variant.id)}<option value={variant.id}
                >{variant.effort || 'default'}</option
              >{/each}</select
          ></label
        >
        <label
          >Default mode<select
            aria-label="Default mode"
            value={$view.fields.mode}
            onchange={(e) => change('mode', e.currentTarget.value)}
            >{#each ['default', 'accept-edits', 'plan', 'bypass'] as mode (mode)}<option
                value={mode}>{mode}</option
              >{/each}</select
          ></label
        >
      </div>
      <label
        >Default folder<input
          value={$view.fields.default_cwd}
          placeholder="~ (your home folder)"
          oninput={(e) => settings.field('default_cwd', e.currentTarget.value)}
          onchange={() => settings.saveField('default_cwd')}
        /><small>Empty: new sessions start in your home folder.</small></label
      >
      <label class="toggle"
        ><input
          type="checkbox"
          checked={$view.fields.worktree_default === 'on'}
          onchange={(e) => change('worktree_default', e.currentTarget.checked ? 'on' : 'off')}
        />Separate git worktree by default</label
      >
      <h3>This computer</h3>
      <label class="toggle"
        ><input
          type="checkbox"
          checked={$view.fields.keep_awake === 'on'}
          onchange={(e) => change('keep_awake', e.currentTarget.checked ? 'on' : 'off')}
        />Keep awake while agents work</label
      >
      {#if native && autostartAvailable}<label class="toggle"
          ><input
            type="checkbox"
            checked={autostart}
            disabled={nativeBusy}
            onchange={(event) => changeAutostart(event.currentTarget.checked)}
          />Open at login</label
        >
        <p>Starts in the tray; your window stays hidden.</p>{:else}<p>
          The display may still turn off. Open at login is available in the desktop app.
        </p>{/if}
      <h3>Notifications</h3>
      <p>Saved on this device. The browser asks for notification permission when needed.</p>
      {#each Object.entries(notificationLabels) as [kind, label] (kind)}<label class="toggle"
          ><input
            type="checkbox"
            checked={$prefs.notify[kind as NotificationKind]}
            onchange={(e) => preferences.notify(kind as NotificationKind, e.currentTarget.checked)}
          />{label}</label
        >{/each}
      <label class="toggle"
        ><input
          type="checkbox"
          checked={$prefs.focused}
          onchange={(e) => preferences.change({ focused: e.currentTarget.checked })}
        />Also while antiquota is in front</label
      >
    {:else}
      <h3>Goals</h3>
      <label
        >Verifier model<select
          value={$view.fields.verifier_model}
          onchange={(e) => change('verifier_model', e.currentTarget.value)}
          >{#each [...models, ...(models.some((m) => m.id === $view.fields!.verifier_model) ? [] : [{ id: $view.fields.verifier_model, label: $view.fields.verifier_model }])] as model (model.id)}<option
              value={model.id}>{model.label}</option
            >{/each}</select
        ></label
      >
      <div class="fields">
        {#each numbers as field (field.key)}<label
            >{field.label}<input
              type="number"
              min={field.min}
              max={field.max}
              value={$view.fields[field.key]}
              oninput={(e) => settings.field(field.key, e.currentTarget.value)}
              onchange={() => settings.saveField(field.key)}
            /></label
          >{/each}
      </div>
      <p>
        A goal is met when agy says GOAL_COMPLETE, your check passes and an independent verifier
        agrees. Claude/GPT verifiers fall back to the session model when their quota runs out.
      </p>
      <h3>Plan mode</h3>
      <p>
        In plan mode agy investigates and proposes a numbered plan; edits are refused. When ready,
        the plan bar offers to execute it.
      </p>
    {/if}
  </section>
{/if}

<style>
  .defaults {
    display: grid;
    gap: var(--space-4);
  }
  h3 {
    font-size: 13px;
    font-weight: 500;
  }
  h3:not(:first-child) {
    margin-top: var(--space-4);
  }
  p,
  small {
    font-size: 11px;
    line-height: 1.7;
    color: var(--ink-3);
  }
  label {
    display: grid;
    gap: var(--space-2);
    font-size: 12px;
    color: var(--ink-2);
  }
  .fields {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--space-3);
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .toggle input {
    width: auto;
    min-width: auto;
  }
  @media (max-width: 760px) {
    .fields {
      grid-template-columns: 1fr;
    }
  }
</style>
