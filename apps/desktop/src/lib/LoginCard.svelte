<script lang="ts">
  // The way in. An invite link (`yappa://invite/…`, from the server's admin) asks for a
  // name and logs in with the control plane. A developer invite (`yappa1.…`, from
  // `cargo xtask livekit token`) still goes straight into its room, as in the test client.
  import { getApp } from './app.svelte';
  import { inviteKind } from './api';
  import Info from './Info.svelte';

  const app = getApp();
  const { session, gang } = app;
  let name = $state('');
  let busy = $state(false);

  const kind = $derived(inviteKind(session.invite));
  const problem = $derived(kind === 'link' ? gang.problem : session.inviteProblem);
  const ready = $derived(
    session.invite.trim() !== '' && (kind !== 'link' || name.trim().length >= 2) && !busy,
  );

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (kind === 'link') {
      busy = true;
      if (await gang.login(session.invite, name)) {
        session.invite = '';
        app.onboarding = 'mic';
      }
      busy = false;
    } else {
      await session.join();
    }
  }
</script>

<form class="card" onsubmit={submit}>
  <h1>Willkommen bei yAPPA</h1>
  <p class="lead">Füge die Einladung ein, die du bekommen hast.</p>
  <div class="label"><label for="invite">Einladung</label><Info term="einladung" /></div>
  <textarea
    id="invite"
    rows="3"
    spellcheck="false"
    placeholder="yappa://invite/…"
    bind:value={session.invite}
    aria-describedby={problem ? 'invite-problem' : undefined}
    aria-invalid={problem ? 'true' : undefined}></textarea>
  {#if kind === 'link'}
    <label for="name">Dein Name</label>
    <input
      id="name"
      type="text"
      autocomplete="nickname"
      maxlength="32"
      placeholder="So sehen dich die anderen"
      bind:value={name}
    />
  {/if}
  {#if problem}
    <p id="invite-problem" class="problem" role="alert">{problem}</p>
  {/if}
  <button type="submit" class="primary" disabled={!ready}>
    {busy || session.connection === 'connecting' ? 'Verbinde …' : 'Beitreten'}
  </button>
  <p class="hint">
    Eine Einladung funktioniert einmal. Danach merkt sich yAPPA dich auf diesem Rechner.
  </p>
</form>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-width: 460px;
  }

  h1 {
    margin: 0;
    font-size: var(--font-size-xl);
  }

  .lead {
    margin: 0 0 var(--space-3);
    color: var(--color-text-muted);
  }

  .label {
    display: flex;
    align-items: center;
  }

  label {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  label[for='name'] {
    margin-top: var(--space-2);
  }

  textarea,
  input {
    font: inherit;
    color: var(--color-text);
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-2);
    user-select: text;
  }

  textarea {
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    resize: vertical;
    word-break: break-all;
  }

  textarea[aria-invalid='true'] {
    border-color: var(--color-danger);
  }

  .primary {
    align-self: flex-start;
    margin-top: var(--space-2);
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: var(--color-accent-text);
    font-weight: 600;
  }

  .primary:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .problem {
    margin: 0;
    color: var(--color-danger);
  }

  .hint {
    margin: var(--space-2) 0 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
</style>
