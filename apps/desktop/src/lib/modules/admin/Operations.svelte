<script lang="ts">
  // Admin → „Betrieb“: the version policy (minimum version, kill switch) and the event log —
  // joins, leaves, kicks, redeemed invites, from LiveKit's webhooks and the control plane.
  import { getApp } from '../../app.svelte';
  import { compareVersions, isBlocked } from '../../version';
  import { stamp } from './time';

  const app = getApp();
  const { admin } = app;

  let minVersion = $state(app.config?.minVersion ?? '0.0.0');
  let killed = $state((app.config?.killedVersions ?? []).join(', '));
  let saved = $state(false);

  const killedList = $derived(
    killed
      .split(/[,\s]+/)
      .map((v) => v.trim())
      .filter(Boolean),
  );
  const valid = $derived(
    compareVersions(minVersion.trim(), '0.0.0') !== null &&
      killedList.every((v) => compareVersions(v, '0.0.0') !== null),
  );
  const mine = $derived(app.info?.version ?? null);
  /** Would this policy lock out the admin's own app? Then say so before saving. */
  const locksMeOut = $derived(
    valid && mine !== null && isBlocked(mine, minVersion.trim(), killedList),
  );

  async function save(event: SubmitEvent) {
    event.preventDefault();
    await admin.setPolicy(minVersion.trim(), killedList);
    saved = !admin.problem;
  }
</script>

<section class="ops" aria-labelledby="ops-title">
  <h1 id="ops-title">Betrieb</h1>

  <form class="policy" onsubmit={save} oninput={() => (saved = false)}>
    <h2>Versionen</h2>
    <label for="min">Mindestversion – ältere Apps bekommen keinen Zugang mehr</label>
    <input id="min" bind:value={minVersion} placeholder="0.1.0" spellcheck="false" />
    <label for="killed">Gesperrte Versionen – müssen den Call verlassen und updaten</label>
    <input id="killed" bind:value={killed} placeholder="z. B. 0.2.1, 0.2.2" spellcheck="false" />
    {#if !valid}
      <p class="warn" role="status">Versionen bitte als Zahlen wie 0.2.0.</p>
    {:else if locksMeOut}
      <p class="warn" role="status">
        Achtung: Damit sperrst du auch deine eigene App ({mine}) aus.
      </p>
    {/if}
    <div class="row">
      <button type="submit" disabled={!valid || admin.busy}>Speichern</button>
      {#if saved}<span class="muted" role="status">Gespeichert – gilt sofort.</span>{/if}
    </div>
  </form>

  <h2>Ereignisse</h2>
  <button class="quiet" disabled={admin.busy} onclick={() => admin.refresh()}>Neu laden</button>
  <ol class="log" aria-label="Ereignis-Log">
    {#each admin.events as event, i (i)}
      <li>
        <time datetime={event.time}>{stamp(event.time)}</time>
        <span>{event.message}</span>
      </li>
    {:else}
      <li class="muted">{admin.busy ? 'Lade …' : 'Noch nichts passiert.'}</li>
    {/each}
  </ol>
</section>

<style>
  .ops {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-width: 720px;
  }

  h1 {
    margin: 0 0 var(--space-2);
    font-size: var(--font-size-lg);
  }

  h2 {
    margin: var(--space-3) 0 0;
    font-size: var(--font-size-md);
  }

  .policy {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    max-width: 480px;
  }

  label {
    margin-top: var(--space-2);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  input {
    font-family: var(--font-mono);
    color: var(--color-text);
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-1) var(--space-2);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-top: var(--space-2);
  }

  .warn {
    margin: 0;
    color: var(--color-warning);
    font-size: var(--font-size-sm);
  }

  .muted {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .quiet {
    align-self: flex-start;
    background: transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .log {
    list-style: none;
    margin: 0;
    padding: 0;
    font-size: var(--font-size-sm);
  }

  .log li {
    display: flex;
    gap: var(--space-3);
    padding: var(--space-1) 0;
    border-bottom: 1px solid var(--color-border);
  }

  time {
    flex: none;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
  }
</style>
