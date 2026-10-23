<script lang="ts">
  // Admin → „Leute“: every member with role, where they are right now, their app version and
  // when the server last saw them. Kicking takes someone out of every room — they stay a
  // member and can come back; the version below the minimum is marked.
  import { getApp } from '../../app.svelte';
  import type { Role } from '../../types/Role';
  import { isBlocked } from '../../version';
  import { since } from './time';

  const app = getApp();
  const { admin, gang } = app;
  let confirming = $state<string | null>(null);

  const where = (userId: string): string => {
    const entry = gang.people.find((p) => p.userId === userId);
    if (!entry) return 'offline';
    const channel = gang.channelName(entry.state.channelId);
    const place = channel ? `in ${channel}` : 'online';
    return entry.state.away ? `${place} · abwesend` : place;
  };

  const outdated = (version: string) =>
    app.config !== null && isBlocked(version, app.config.minVersion, app.config.killedVersions);

  async function kick(userId: string) {
    if (confirming !== userId) {
      confirming = userId;
      return;
    }
    confirming = null;
    await admin.kick(userId);
  }
</script>

<section aria-labelledby="people-title">
  <h1 id="people-title">Leute</h1>
  <table>
    <thead>
      <tr>
        <th scope="col">Name</th>
        <th scope="col">Rolle</th>
        <th scope="col">Gerade</th>
        <th scope="col">Version</th>
        <th scope="col">Zuletzt gesehen</th>
        <th scope="col"><span class="visually-hidden">Aktionen</span></th>
      </tr>
    </thead>
    <tbody>
      {#each admin.members as member (member.user.id)}
        {@const me = member.user.id === gang.me?.id}
        <tr>
          <th scope="row">{member.user.displayName}{me ? ' (du)' : ''}</th>
          <td>
            <select
              aria-label="Rolle von {member.user.displayName}"
              value={member.user.role}
              disabled={me || admin.busy}
              onchange={(e) => admin.setRole(member.user.id, e.currentTarget.value as Role)}
            >
              <option value="member">Mitglied</option>
              <option value="admin">Admin</option>
            </select>
          </td>
          <td>{where(member.user.id)}</td>
          <td>
            {#if member.client}
              <span class:warn={outdated(member.client.appVersion)}>
                {member.client.appVersion}
                {outdated(member.client.appVersion) ? '· zu alt' : ''}
              </span>
              <span class="muted">{member.client.platform}</span>
            {:else}
              <span class="muted">–</span>
            {/if}
          </td>
          <td>{member.lastSeen ? since(member.lastSeen) : '–'}</td>
          <td>
            {#if !me}
              <button
                class="quiet"
                class:danger={confirming === member.user.id}
                disabled={admin.busy}
                onclick={() => kick(member.user.id)}
                onblur={() => (confirming = null)}
              >
                {confirming === member.user.id ? 'Wirklich rauswerfen?' : 'Rauswerfen'}
              </button>
            {/if}
          </td>
        </tr>
      {:else}
        <tr><td colspan="6" class="muted">{admin.busy ? 'Lade …' : 'Noch niemand.'}</td></tr>
      {/each}
    </tbody>
  </table>
  <p class="note">
    Rauswerfen nimmt die Person aus allen Channels. Sie bleibt Mitglied und kann wieder beitreten –
    wer nicht mehr dabei sein soll, bekommt einfach keine neue Einladung mehr.
  </p>
</section>

<style>
  h1 {
    margin: 0 0 var(--space-4);
    font-size: var(--font-size-lg);
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--font-size-sm);
  }

  th,
  td {
    padding: var(--space-2);
    border-bottom: 1px solid var(--color-border);
    text-align: left;
    vertical-align: middle;
  }

  thead th {
    color: var(--color-text-muted);
    font-weight: normal;
  }

  tbody th {
    font-weight: 600;
  }

  select {
    font: inherit;
    color: inherit;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-1);
  }

  .muted {
    color: var(--color-text-muted);
  }

  .warn {
    color: var(--color-warning);
  }

  .quiet {
    background: transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .danger {
    color: var(--color-danger);
    border-color: var(--color-danger);
  }

  .note {
    margin: var(--space-3) 0 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
