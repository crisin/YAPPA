<script lang="ts">
  // Profi → „Tasten“: the global keys — push-to-talk, mute and deafen (A3). All three go
  // through the same hook, so they work while a game has the focus; one key does one thing.
  import Info from '../../Info.svelte';
  import KeyPicker from './KeyPicker.svelte';
  import PttKey from './PttKey.svelte';
</script>

<section class="keys" aria-labelledby="keys-title">
  <h1 id="keys-title">Tasten</h1>
  <section aria-labelledby="keys-ptt">
    <h2 id="keys-ptt">Push-to-Talk <Info term="push-to-talk" /></h2>
    <PttKey />
    <p class="note">Die Taste gilt im Modus „Taste halten“.</p>
  </section>
  <section aria-labelledby="keys-mute">
    <h2 id="keys-mute">Stumm <Info term="stumm-und-taub" /></h2>
    <KeyPicker
      action="mute"
      note="Ein Druck schaltet dein Mikrofon aus, der nächste wieder an – auch mitten im Spiel. Das Dock und die anderen sehen es."
    />
  </section>
  <section aria-labelledby="keys-deafen">
    <h2 id="keys-deafen">Taub</h2>
    <KeyPicker
      action="deafen"
      note="Ein Druck schaltet alles aus: du hörst nichts und sendest nichts. Der nächste Druck gibt dir Ton und Mikrofon zurück, wie sie vorher waren."
    />
  </section>
</section>

<style>
  .keys {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    max-width: 480px;
  }

  h1 {
    margin: 0;
    font-size: var(--font-size-lg);
  }

  h2 {
    display: flex;
    align-items: center;
    margin: 0;
    font-size: var(--font-size-md);
  }

  .note {
    margin: var(--space-2) 0 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
</style>
