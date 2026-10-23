<script lang="ts">
  // Profi → „Tasten“: the global keys. Today only push-to-talk; mute and deafen follow.
  import Info from '../../Info.svelte';
  import PttKey from './PttKey.svelte';
</script>

<section class="keys" aria-labelledby="keys-title">
  <h1 id="keys-title">Tasten</h1>
  <h2>Push-to-Talk <Info term="push-to-talk" /></h2>
  <PttKey />
  <p class="note">
    Die Taste gilt im Modus „Taste halten“. Tasten für Stummschalten und Taubschalten kommen später.
  </p>
</section>

<style>
  .keys {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-width: 480px;
  }

  h1 {
    margin: 0 0 var(--space-2);
    font-size: var(--font-size-lg);
  }

  h2 {
    display: flex;
    align-items: center;
    margin: 0;
    font-size: var(--font-size-md);
  }

  .note {
    margin: var(--space-3) 0 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
</style>
