<script lang="ts">
  // „Konto“: who I am to the gang, and the way out of this machine's login.
  import { getApp } from '../../app.svelte';

  const app = getApp();
  const { gang } = app;
  let confirm = $state(false);

  async function logout() {
    confirm = false;
    await gang.logout();
    app.shell.open('voice');
  }
</script>

<section aria-labelledby="account-title">
  <h1 id="account-title">Konto</h1>
  {#if gang.status === 'in' && gang.me}
    <dl>
      <dt>Name</dt>
      <dd>{gang.me.displayName}</dd>
      <dt>Rolle</dt>
      <dd>{gang.me.role === 'admin' ? 'Admin' : 'Mitglied'}</dd>
    </dl>
    {#if confirm}
      <p>Abmelden heißt: Für den nächsten Start brauchst du eine neue Einladung.</p>
      <div class="row">
        <button class="danger" onclick={logout}>Ja, abmelden</button>
        <button onclick={() => (confirm = false)}>Doch nicht</button>
      </div>
    {:else}
      <button onclick={() => (confirm = true)}>Abmelden …</button>
    {/if}
  {:else}
    <p class="quiet">Nicht angemeldet. Die Einladung fügst du unter „Voice“ ein.</p>
  {/if}
</section>

<style>
  h1 {
    margin: 0 0 var(--space-4);
    font-size: var(--font-size-lg);
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-2) var(--space-4);
    margin: 0 0 var(--space-4);
  }

  dt,
  .quiet {
    color: var(--color-text-muted);
  }

  dd {
    margin: 0;
  }

  .row {
    display: flex;
    gap: var(--space-2);
  }

  .danger {
    border-color: var(--color-danger);
    color: var(--color-danger);
  }
</style>
