<script lang="ts">
  // `Strg K`: every module registers its targets and actions, the palette offers them all.
  // Bits UI brings filtering, arrow keys and the dialog's focus trap; the look is ours.
  import { Command, Dialog } from 'bits-ui';
  import { getApp } from '../app.svelte';
  import type { CommandDef } from '../shell.svelte';

  const { shell } = getApp();

  const groups = $derived.by(() => {
    if (!shell.palette.open) return [];
    const byGroup = new Map<string, CommandDef[]>();
    for (const command of shell.commands()) {
      byGroup.set(command.group, [...(byGroup.get(command.group) ?? []), command]);
    }
    return [...byGroup];
  });

  function run(command: CommandDef) {
    shell.closePalette();
    command.run();
  }
</script>

<Dialog.Root
  open={shell.palette.open}
  onOpenChange={(open) => (open ? shell.openPalette() : shell.closePalette())}
>
  <Dialog.Portal>
    <Dialog.Overlay class="palette-overlay" />
    <Dialog.Content class="palette" aria-describedby={undefined}>
      <Dialog.Title class="visually-hidden">Befehlspalette</Dialog.Title>
      <Command.Root label="Befehle" loop>
        <Command.Input
          class="palette-input"
          placeholder="Wohin oder was tun …"
          aria-label="Suchen"
        />
        <Command.List class="palette-list">
          <Command.Viewport>
            <Command.Empty class="palette-empty">Nichts gefunden.</Command.Empty>
            {#each groups as [group, commands] (group)}
              <Command.Group value={group}>
                <Command.GroupHeading class="palette-heading">{group}</Command.GroupHeading>
                <Command.GroupItems>
                  {#each commands as command (command.id)}
                    <Command.Item
                      class="palette-item"
                      value={command.id}
                      keywords={[command.label, ...(command.keywords ?? [])]}
                      onSelect={() => run(command)}
                    >
                      {command.label}
                    </Command.Item>
                  {/each}
                </Command.GroupItems>
              </Command.Group>
            {/each}
          </Command.Viewport>
        </Command.List>
      </Command.Root>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  /* Bits UI renders these elements itself, so the classes are global. */
  :global(.palette-overlay) {
    position: fixed;
    inset: 0;
    background: var(--color-overlay);
  }

  :global(.palette) {
    position: fixed;
    top: 15vh;
    left: 50%;
    transform: translateX(-50%);
    width: min(560px, 90vw);
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    overflow: hidden;
    color: var(--color-text);
  }

  :global(.palette-input) {
    width: 100%;
    font: inherit;
    color: inherit;
    padding: var(--space-3) var(--space-4);
    background: transparent;
    border: 0;
    border-bottom: 1px solid var(--color-border);
    outline: none;
  }

  :global(.palette-list) {
    max-height: 50vh;
    overflow-y: auto;
    padding: var(--space-2);
  }

  :global(.palette-heading) {
    padding: var(--space-2) var(--space-2) var(--space-1);
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  :global(.palette-item) {
    padding: var(--space-2);
    border-radius: var(--radius-md);
    cursor: pointer;
  }

  :global(.palette-item[data-selected]) {
    background: var(--color-surface-raised);
  }

  :global(.palette-empty) {
    padding: var(--space-4);
    color: var(--color-text-muted);
  }
</style>
