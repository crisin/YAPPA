<script lang="ts" generics="T extends string">
  // A row of program buttons as on a hardware unit: one is lit. Semantically a radio group,
  // so the arrow keys walk it and a screen reader says "1 of 5".
  let {
    label,
    options,
    value,
    onchange,
  }: {
    label: string;
    options: readonly { id: T; label: string }[];
    value: T;
    onchange: (id: T) => void;
  } = $props();

  function key(e: KeyboardEvent, index: number) {
    const delta =
      e.key === 'ArrowRight' || e.key === 'ArrowDown'
        ? 1
        : e.key === 'ArrowLeft' || e.key === 'ArrowUp'
          ? -1
          : 0;
    if (!delta) return;
    e.preventDefault();
    const next = options[(index + delta + options.length) % options.length];
    onchange(next.id);
    (e.currentTarget as HTMLElement).parentElement
      ?.querySelectorAll<HTMLElement>('[role="radio"]')
      [(index + delta + options.length) % options.length]?.focus();
  }
</script>

<div class="programs" role="radiogroup" aria-label={label}>
  {#each options as o, i (o.id)}
    <button
      type="button"
      role="radio"
      aria-checked={o.id === value}
      tabindex={o.id === value ? 0 : -1}
      onclick={() => onchange(o.id)}
      onkeydown={(e) => key(e, i)}
    >
      <span class="led" aria-hidden="true"></span>
      {o.label}
    </button>
  {/each}
</div>

<style>
  .programs {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
  }

  button {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) var(--space-3) var(--space-1) var(--space-2);
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    background: linear-gradient(var(--color-surface-raised), var(--color-surface));
    border: 1px solid var(--color-knob-edge);
    border-radius: var(--radius-sm);
    box-shadow: 0 1px 1px rgba(0, 0, 0, 0.3);
  }

  button[aria-checked='true'] {
    color: var(--color-text);
    background: var(--color-surface-sunken);
    box-shadow: inset 0 1px 3px rgba(0, 0, 0, 0.4);
  }

  .led {
    width: 7px;
    height: 7px;
    border-radius: var(--radius-full);
    background: var(--color-state-offline);
    transition: background var(--duration-fast) var(--ease-standard);
  }

  button[aria-checked='true'] .led {
    background: var(--color-speaking);
    box-shadow: 0 0 6px var(--color-speaking);
  }
</style>
