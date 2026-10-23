<script lang="ts">
  // The first start, full window, before the shell (concept: „Onboarding per Invite“):
  // 1 Name — the login card (invite + name), 2 Mikro — device with a live level,
  // 3 Reden — headset or speakers, then the speaking mode. Afterwards straight into the lobby.
  // Steps 2 and 3 are the same parts as „Ich → Audio und Sprechen“.
  import { getApp } from './app.svelte';
  import LoginCard from './LoginCard.svelte';
  import MicTest from './modules/me/MicTest.svelte';
  import TalkMode from './modules/me/TalkMode.svelte';
  import BannerSlot from './shell/BannerSlot.svelte';

  const app = getApp();
  const { session, gang } = app;

  const step = $derived(app.onboarding === 'talk' ? 3 : app.onboarding === 'mic' ? 2 : 1);
  const steps = ['Name', 'Mikro', 'Reden'];

  /** Headset or speakers: speakers send the others back to themselves (no echo cancelling in
   * v1), so „Taste halten“ is the advice there. */
  let listening = $state<'headset' | 'speakers' | null>(null);
  let finishing = $state(false);

  const deviceValue = (id: string | null | undefined) => id ?? '';
  const deviceId = (value: string) => (value === '' ? null : value);

  function chooseListening(choice: 'headset' | 'speakers') {
    listening = choice;
    void app.setTransmitMode(choice === 'speakers' ? 'pushToTalk' : 'voiceActivation');
  }

  async function finish() {
    finishing = true;
    try {
      await app.finishOnboarding();
    } finally {
      finishing = false;
    }
  }
</script>

<div class="onboarding">
  <BannerSlot />
  <main class="center" aria-label="Einrichtung">
    <ol class="progress" aria-label="Schritte">
      {#each steps as label, i (label)}
        <li aria-current={step === i + 1 ? 'step' : undefined} class:done={step > i + 1}>
          <span class="dot">{i + 1}</span>
          {label}
        </li>
      {/each}
    </ol>

    {#if step === 1}
      <LoginCard />
    {:else if step === 2}
      <section class="card" aria-labelledby="mic-title">
        <h1 id="mic-title">Hört dich yAPPA?</h1>
        <p class="lead">
          Hallo {gang.me?.displayName}! Sag etwas – der Balken sollte sich mit deiner Stimme
          bewegen. Wenn nicht, nimm ein anderes Mikrofon.
        </p>
        <MicTest note={false} />
        <label for="onboarding-output">Ausgabe (Kopfhörer oder Lautsprecher)</label>
        <select
          id="onboarding-output"
          value={deviceValue(session.settings?.outputDevice)}
          onchange={(e) => session.setOutput(deviceId(e.currentTarget.value))}
        >
          <option value="">Standardgerät</option>
          {#each session.outputs as device (device.id)}
            <option value={device.id}>{device.name}</option>
          {/each}
        </select>
        <div class="actions">
          <button class="primary" onclick={() => (app.onboarding = 'talk')}>Weiter</button>
          <button class="quiet" onclick={() => session.refreshDevices()}>Geräte neu einlesen</button
          >
        </div>
      </section>
    {:else}
      <section class="card" aria-labelledby="talk-title">
        <h1 id="talk-title">Wie redest du?</h1>
        <fieldset>
          <legend>Was hast du auf den Ohren?</legend>
          <label class="choice">
            <input
              type="radio"
              name="listening"
              checked={listening === 'headset'}
              onchange={() => chooseListening('headset')}
            />
            <span>
              <strong>Headset oder Kopfhörer</strong>
              <span class="note">Dann geht dein Mikro automatisch an, wenn du sprichst.</span>
            </span>
          </label>
          <label class="choice">
            <input
              type="radio"
              name="listening"
              checked={listening === 'speakers'}
              onchange={() => chooseListening('speakers')}
            />
            <span>
              <strong>Lautsprecher</strong>
              <span class="note">
                Dann hört dein Mikro die anderen mit, und sie hören sich selbst als Echo. Mit „Taste
                halten“ passiert das nur, während du sprichst.
              </span>
            </span>
          </label>
        </fieldset>
        {#if listening}
          <TalkMode />
        {/if}
        <div class="actions">
          <button class="primary" onclick={finish} disabled={listening === null || finishing}>
            {finishing ? 'Verbinde …' : 'Ab in die Lobby'}
          </button>
          <button class="quiet" onclick={() => (app.onboarding = 'mic')}>Zurück</button>
        </div>
        <p class="note">Alles lässt sich später unter „Ich → Audio und Sprechen“ ändern.</p>
      </section>
    {/if}
  </main>
</div>

<style>
  .onboarding {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--color-bg);
    overflow: auto;
  }

  .center {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-5);
    margin: auto;
    padding: var(--space-4);
    width: 100%;
    max-width: 520px;
  }

  .progress {
    display: flex;
    gap: var(--space-4);
    list-style: none;
    margin: 0;
    padding: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .progress li {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  .progress li[aria-current='step'] {
    color: var(--color-text);
    font-weight: 600;
  }

  .dot {
    display: inline-grid;
    place-items: center;
    width: 1.5em;
    height: 1.5em;
    border: 1px solid var(--color-border);
    border-radius: 50%;
  }

  [aria-current='step'] .dot {
    border-color: var(--color-accent);
    background: var(--color-accent);
    color: var(--color-accent-text);
  }

  .done .dot {
    border-color: var(--color-accent);
  }

  .center > :global(*:last-child) {
    width: 100%;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  h1 {
    margin: 0;
    font-size: var(--font-size-xl);
  }

  .lead {
    margin: 0 0 var(--space-2);
    color: var(--color-text-muted);
  }

  label[for] {
    margin-top: var(--space-3);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  select {
    font: inherit;
    color: inherit;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-2);
  }

  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-3) var(--space-4);
  }

  legend {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .choice {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
  }

  .choice span {
    display: flex;
    flex-direction: column;
  }

  input {
    accent-color: var(--color-accent);
  }

  .note {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    font-weight: normal;
  }

  .actions {
    display: flex;
    gap: var(--space-3);
    align-items: center;
    margin-top: var(--space-4);
  }

  .primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: var(--color-accent-text);
    font-weight: 600;
  }

  .primary:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .quiet {
    background: transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
</style>
