<script lang="ts">
  // A small ⓘ next to a label: hover or click shows the glossary's one-sentence explanation,
  // "Mehr im Glossar" opens the full entry in the help. `term` is a slug of
  // docs/hilfe/glossar.md (a `##` heading); an unknown term renders nothing.
  import { glossary, GLOSSARY_PAGE } from './docs';
  import { help } from './helpState.svelte';

  let { term }: { term: string } = $props();

  const entry = $derived(glossary[term]);
  const id = $derived(`info-${term}`);
  let open = $state(false);
  let mark = $state<HTMLButtonElement | null>(null);
  // Fixed position from the mark's rectangle: the bubble must escape the scroll clipping of
  // the sidebar and the debug panel, and stay inside the window.
  let top = $state(0);
  let left = $state(0);
  const WIDTH = 300;

  function show() {
    const rect = mark?.getBoundingClientRect();
    if (rect) {
      top = rect.bottom + 4;
      left = Math.max(8, Math.min(rect.left, window.innerWidth - WIDTH - 8));
    }
    open = true;
  }

  function more() {
    open = false;
    help.show(GLOSSARY_PAGE, term);
  }
</script>

{#if entry}
  <span
    class="info"
    onmouseenter={show}
    onmouseleave={() => (open = false)}
    onkeydown={(e) => {
      // Escape closes the tooltip and nothing else (the shell steps back on a free Escape).
      if (e.key === 'Escape' && open) {
        e.preventDefault();
        open = false;
      }
    }}
    role="presentation"
  >
    <button
      type="button"
      class="mark"
      aria-label="Erklärung: {entry.term}"
      aria-expanded={open}
      aria-controls={id}
      onclick={show}
      bind:this={mark}
    >
      i
    </button>
    {#if open}
      <span class="bubble" {id} role="tooltip" style:top="{top}px" style:left="{left}px">
        <strong>{entry.term}</strong>
        {entry.short}
        <button type="button" class="more" onclick={more}>Mehr im Glossar</button>
      </span>
    {/if}
  </span>
{/if}

<style>
  .info {
    position: relative;
    display: inline-block;
    margin-left: var(--space-1);
    vertical-align: middle;
  }

  .mark {
    width: 16px;
    height: 16px;
    padding: 0;
    border-radius: 50%;
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 700;
    line-height: 1;
    color: var(--color-text-muted);
    background: transparent;
  }

  .mark[aria-expanded='true'] {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }

  .bubble {
    position: fixed;
    z-index: 10;
    width: max-content;
    max-width: min(300px, 80vw);
    padding: var(--space-2) var(--space-3);
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    box-shadow: 0 6px 20px rgb(0 0 0 / 30%);
    color: var(--color-text);
    font-family: var(--font-sans);
    font-size: var(--font-size-sm);
    font-weight: 400;
    line-height: 1.45;
    text-align: left;
    text-transform: none;
    letter-spacing: normal;
    user-select: text;
  }

  .bubble strong {
    display: block;
    margin-bottom: 2px;
  }

  .more {
    display: block;
    margin-top: var(--space-2);
    padding: 0;
    background: transparent;
    border: 0;
    color: var(--color-accent);
    font-size: var(--font-size-sm);
  }
</style>
