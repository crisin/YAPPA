<script lang="ts">
  // The help in the window: pages on the left (two groups, or search hits), the page on
  // the right. Links between pages (`doc:`) stay inside; web links open in the browser;
  // `repo:` links are paths in the repository and only shown.
  import { tick } from 'svelte';
  import { engine } from './engine';
  import { GROUPS, pageById, pagesIn, search, type Page } from './docs';
  import { help } from './helpState.svelte';

  let query = $state('');
  let article = $state<HTMLElement | null>(null);

  const page = $derived(pageById(help.page) ?? pagesIn(GROUPS[0].id)[0]);
  const hits = $derived(search(query));

  function show(target: Page, anchor?: string) {
    query = '';
    help.show(target.id, anchor);
  }

  function follow(event: MouseEvent) {
    const link = (event.target as HTMLElement).closest('a');
    if (!link) return;
    event.preventDefault();
    const href = link.getAttribute('href') ?? '';
    if (href.startsWith('doc:')) {
      const [id, anchor] = href.slice(4).split('#');
      const target = pageById(id);
      if (target) show(target, anchor);
    } else if (href.startsWith('#')) {
      help.anchor = href.slice(1);
    } else if (/^https?:\/\//.test(href)) {
      void engine.openUrl(href);
    }
  }

  // Clicks on the rendered links are handled here (the HTML is not Svelte markup).
  $effect(() => {
    const node = article;
    node?.addEventListener('click', follow);
    return () => node?.removeEventListener('click', follow);
  });

  // Scroll to the heading once the page is in the DOM; without an anchor start at the top.
  $effect(() => {
    const anchor = help.anchor;
    const id = page?.id;
    void tick().then(() => {
      if (!article || !id) return;
      const heading = anchor ? article.querySelector(`#${CSS.escape(anchor)}`) : null;
      if (heading) heading.scrollIntoView?.({ block: 'start' });
      else article.scrollTo?.(0, 0);
    });
  });
</script>

<div class="help">
  <nav aria-label="Hilfe-Seiten">
    <input
      type="search"
      placeholder="Suchen …"
      aria-label="In der Hilfe suchen"
      bind:value={query}
    />
    {#if query.trim()}
      <h2>{hits.length === 1 ? '1 Treffer' : `${hits.length} Treffer`}</h2>
      <ul>
        {#each hits as hit (hit.id)}
          <li><button class="page" onclick={() => show(hit)}>{hit.title}</button></li>
        {/each}
      </ul>
    {:else}
      {#each GROUPS as group (group.id)}
        <h2>{group.label}</h2>
        <ul>
          {#each pagesIn(group.id) as entry (entry.id)}
            <li>
              <button
                class="page"
                aria-current={entry.id === page?.id ? 'page' : undefined}
                onclick={() => show(entry)}
              >
                {entry.title}
              </button>
            </li>
          {/each}
        </ul>
      {/each}
    {/if}
  </nav>

  {#if page}
    <article class="prose" bind:this={article}>
      <!-- generated from our own markdown by cargo xtask docs -->
      {@html page.html}
      <p class="source">Quelle: <code>{page.source}</code></p>
    </article>
  {/if}
</div>

<style>
  .help {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: var(--space-6);
    height: 100%;
    min-height: 0;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    overflow-y: auto;
    padding-right: var(--space-2);
    border-right: 1px solid var(--color-border);
  }

  input {
    font: inherit;
    color: inherit;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-2);
    margin-bottom: var(--space-2);
  }

  nav h2 {
    margin: var(--space-3) 0 var(--space-1);
    font-size: var(--font-size-sm);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--color-text-muted);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .page {
    width: 100%;
    text-align: left;
    background: transparent;
    border-color: transparent;
    padding: var(--space-1) var(--space-2);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .page:hover {
    color: var(--color-text);
  }

  .page[aria-current='page'] {
    background: var(--color-surface-raised);
    color: var(--color-text);
  }

  .prose {
    overflow-y: auto;
    padding-right: var(--space-4);
    user-select: text;
    line-height: 1.55;
    scrollbar-color: var(--color-border) transparent;
  }

  .source {
    margin-top: var(--space-6);
    padding-top: var(--space-3);
    border-top: 1px solid var(--color-border);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  /* The rendered markdown is not Svelte markup, so its styles are global to this block. */
  .prose :global(h1) {
    margin: 0 0 var(--space-4);
    font-size: var(--font-size-lg);
  }

  .prose :global(h2) {
    margin: var(--space-6) 0 var(--space-2);
    font-size: var(--font-size-md);
    font-weight: 700;
  }

  .prose :global(h3) {
    margin: var(--space-4) 0 var(--space-2);
    font-size: var(--font-size-md);
    font-weight: 600;
  }

  .prose :global(p),
  .prose :global(ul),
  .prose :global(ol) {
    margin: 0 0 var(--space-3);
  }

  .prose :global(li) {
    margin-bottom: var(--space-1);
  }

  .prose :global(a) {
    color: var(--color-accent);
  }

  .prose :global(a[href^='repo:']) {
    color: inherit;
    text-decoration: underline dotted;
    cursor: default;
  }

  .prose :global(code) {
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    background: var(--color-surface);
    border-radius: var(--radius-sm);
    padding: 1px var(--space-1);
  }

  .prose :global(pre) {
    overflow-x: auto;
    margin: 0 0 var(--space-3);
    padding: var(--space-3);
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
  }

  .prose :global(pre code) {
    background: none;
    padding: 0;
  }

  .prose :global(table) {
    display: block;
    overflow-x: auto;
    border-collapse: collapse;
    margin: 0 0 var(--space-4);
    font-size: var(--font-size-sm);
  }

  .prose :global(th),
  .prose :global(td) {
    text-align: left;
    vertical-align: top;
    padding: var(--space-1) var(--space-3) var(--space-1) 0;
    border-bottom: 1px solid var(--color-border);
  }

  .prose :global(th) {
    color: var(--color-text-muted);
    font-weight: 600;
  }

  .prose :global(blockquote) {
    margin: 0 0 var(--space-3);
    padding-left: var(--space-3);
    border-left: 2px solid var(--color-border);
    color: var(--color-text-muted);
  }

  .prose :global(hr) {
    border: 0;
    border-top: 1px solid var(--color-border);
    margin: var(--space-4) 0;
  }

  .prose :global(input[type='checkbox']) {
    margin-right: var(--space-1);
    pointer-events: none;
  }
</style>
