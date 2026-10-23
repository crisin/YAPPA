<script lang="ts">
  // Zone B of the admin module: its sections.
  import { ADMIN_SECTIONS } from '../../admin.svelte';
  import { getApp } from '../../app.svelte';

  const { admin } = getApp();
</script>

<nav class="sidebar" aria-label="Verwaltung">
  <h2>Admin</h2>
  <ul>
    {#each ADMIN_SECTIONS as section (section.id)}
      <li>
        <button
          aria-current={admin.section === section.id ? 'page' : undefined}
          onclick={() => admin.open(section.id)}
        >
          {section.label}
        </button>
      </li>
    {/each}
  </ul>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: var(--space-3);
  }

  h2 {
    margin: 0 0 var(--space-4);
    padding: var(--space-1) var(--space-2);
    font-size: var(--font-size-lg);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  button {
    width: 100%;
    text-align: left;
    background: transparent;
    border-color: transparent;
  }

  button[aria-current='page'] {
    background: var(--color-surface-raised);
    font-weight: 600;
  }
</style>
