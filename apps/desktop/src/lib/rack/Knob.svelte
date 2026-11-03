<script lang="ts">
  // A rotary knob as on a rack unit: a 270° sweep, an arc shows where it stands, the number
  // underneath says it exactly. Drag up or down (Shift = fine), turn the wheel, use the
  // arrow keys, double-click for the default, click the number to type one in.
  //
  // Controlled: `value` comes from the settings, `onchange` goes back to them. While a drag
  // runs the knob shows its own position and sends at most one value per frame, so the
  // engine is not flooded with every pointer move.
  let {
    id,
    label,
    value,
    min,
    max,
    step = 1,
    defaultValue,
    log = false,
    bipolar = false,
    format = (v: number) => String(v),
    onchange,
    size = 56,
    disabled = false,
  }: {
    id: string;
    label: string;
    value: number;
    min: number;
    max: number;
    step?: number;
    /** Double-click puts the knob here. */
    defaultValue: number;
    /** Logarithmic taper (frequencies, Q): equal turns are equal ratios. */
    log?: boolean;
    /** The arc starts at zero (gain knobs) instead of at the left stop. */
    bipolar?: boolean;
    format?: (v: number) => string;
    onchange: (value: number) => void;
    size?: number;
    disabled?: boolean;
  } = $props();

  const SWEEP = 270;
  const START = -135;

  let live = $state<number | null>(null);
  const shown = $derived(live ?? value);

  const toPos = (v: number) =>
    log ? (Math.log(v) - Math.log(min)) / (Math.log(max) - Math.log(min)) : (v - min) / (max - min);
  const fromPos = (p: number) =>
    log ? Math.exp(Math.log(min) + p * (Math.log(max) - Math.log(min))) : min + p * (max - min);

  /** Onto the step grid and inside the stops; the rounding kills float dust like 2.9999. */
  function quantize(v: number): number {
    const q = Math.round((v - min) / step) * step + min;
    return Number(Math.min(max, Math.max(min, q)).toFixed(6));
  }

  const angle = (p: number) => START + Math.min(1, Math.max(0, p)) * SWEEP;
  const c = $derived(size / 2);
  const r = $derived(size / 2 - 5);

  function point(deg: number, radius: number): string {
    const a = (deg * Math.PI) / 180;
    return `${(c + radius * Math.sin(a)).toFixed(2)} ${(c - radius * Math.cos(a)).toFixed(2)}`;
  }

  function arc(from: number, to: number): string {
    if (Math.abs(to - from) < 0.01) return '';
    const large = Math.abs(to - from) > 180 ? 1 : 0;
    const sweep = to > from ? 1 : 0;
    return `M ${point(from, r)} A ${r} ${r} 0 ${large} ${sweep} ${point(to, r)}`;
  }

  const track = $derived(arc(START, START + SWEEP));
  const current = $derived(angle(toPos(shown)));
  const origin = $derived(bipolar ? angle(toPos(0)) : START);
  const fill = $derived(arc(origin, current));
  const pointer = $derived(`M ${point(current, r * 0.35)} L ${point(current, r * 0.78)}`);

  // Sending: one value per animation frame during a drag, the last one on release.
  let pending: number | null = null;
  let frame = 0;
  function send(v: number) {
    const q = quantize(v);
    live = q;
    pending = q;
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      flush();
    });
  }
  function flush() {
    if (pending === null) return;
    const v = pending;
    pending = null;
    if (v !== value) onchange(v);
  }
  function set(v: number) {
    const q = quantize(v);
    live = null;
    if (q !== value) onchange(q);
  }

  // Drag: 240 px of travel is the full sweep, with Shift a tenth of that.
  let dial = $state<HTMLDivElement | null>(null);
  let drag: { y: number; pos: number; pointer: number } | null = null;

  function down(e: PointerEvent) {
    if (disabled || e.button !== 0) return;
    drag = { y: e.clientY, pos: toPos(shown), pointer: e.pointerId };
    dial?.setPointerCapture?.(e.pointerId);
    dial?.focus();
    e.preventDefault();
  }
  function move(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointer) return;
    const scale = e.shiftKey ? 2400 : 240;
    send(fromPos(drag.pos + (drag.y - e.clientY) / scale));
  }
  function up(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointer) return;
    drag = null;
    dial?.releasePointerCapture?.(e.pointerId);
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    flush();
    live = null;
  }

  /** One notch: a step on a linear knob, a hundredth of the sweep on a log one. */
  function nudge(direction: 1 | -1, big = false) {
    if (disabled) return;
    const n = big ? 10 : 1;
    set(log ? fromPos(toPos(shown) + direction * 0.01 * n) : shown + direction * step * n);
  }

  function key(e: KeyboardEvent) {
    const big = e.shiftKey;
    switch (e.key) {
      case 'ArrowUp':
      case 'ArrowRight':
        nudge(1, big);
        break;
      case 'ArrowDown':
      case 'ArrowLeft':
        nudge(-1, big);
        break;
      case 'PageUp':
        nudge(1, true);
        break;
      case 'PageDown':
        nudge(-1, true);
        break;
      case 'Home':
        set(min);
        break;
      case 'End':
        set(max);
        break;
      default:
        return;
    }
    e.preventDefault();
  }

  // The wheel must not scroll the page under the knob; Svelte attaches `onwheel` passively,
  // so the listener goes on by hand.
  $effect(() => {
    const el = dial;
    if (!el) return;
    const wheel = (e: WheelEvent) => {
      if (disabled) return;
      e.preventDefault();
      if (e.deltaY !== 0) nudge(e.deltaY < 0 ? 1 : -1, e.shiftKey);
    };
    el.addEventListener('wheel', wheel, { passive: false });
    return () => el.removeEventListener('wheel', wheel);
  });

  // Typing a value: click the number, type, Enter. Comma or point both work.
  let editing = $state(false);
  let typed = $state('');
  let field = $state<HTMLInputElement | null>(null);

  function edit() {
    if (disabled) return;
    typed = String(shown).replace('.', ',');
    editing = true;
    queueMicrotask(() => field?.select());
  }
  function commitTyped() {
    if (!editing) return;
    editing = false;
    const v = Number(typed.trim().replace(',', '.'));
    if (Number.isFinite(v)) set(v);
  }
  function fieldKey(e: KeyboardEvent) {
    if (e.key === 'Enter') commitTyped();
    else if (e.key === 'Escape') editing = false;
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
</script>

<div class="knob" class:disabled style:--size="{size}px">
  <div
    bind:this={dial}
    class="dial"
    class:dragging={live !== null}
    role="slider"
    tabindex={disabled ? -1 : 0}
    aria-label={label}
    aria-valuemin={min}
    aria-valuemax={max}
    aria-valuenow={shown}
    aria-valuetext={format(shown)}
    aria-disabled={disabled || undefined}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
    onkeydown={key}
    ondblclick={() => set(defaultValue)}
  >
    <svg viewBox="0 0 {size} {size}" aria-hidden="true">
      <path class="track" d={track} />
      <path class="fill" d={fill} />
      <circle class="face" cx={c} cy={c} r={r * 0.82} />
      <path class="pointer" d={pointer} />
    </svg>
  </div>
  <span class="label" id="{id}-label">{label}</span>
  {#if editing}
    <input
      bind:this={field}
      bind:value={typed}
      class="field"
      type="text"
      inputmode="decimal"
      aria-label="{label} eingeben"
      onkeydown={fieldKey}
      onblur={commitTyped}
    />
  {:else}
    <button
      class="value"
      type="button"
      {disabled}
      aria-label="{label}: {format(shown)} – Wert eingeben"
      onclick={edit}
    >
      {format(shown)}
    </button>
  {/if}
</div>

<style>
  .knob {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    width: calc(var(--size) + 24px);
    user-select: none;
  }

  .dial {
    width: var(--size);
    height: var(--size);
    border-radius: var(--radius-full);
    cursor: ns-resize;
    touch-action: none;
  }

  .dial:focus-visible {
    outline: var(--focus-ring) solid var(--color-focus);
    outline-offset: 2px;
  }

  svg {
    display: block;
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  .track {
    fill: none;
    stroke: var(--color-border);
    stroke-width: 3;
    stroke-linecap: round;
  }

  .fill {
    fill: none;
    stroke: var(--color-accent);
    stroke-width: 3;
    stroke-linecap: round;
  }

  .face {
    fill: var(--color-knob);
    stroke: var(--color-knob-edge);
    stroke-width: 1;
    filter: drop-shadow(0 2px 2px rgba(0, 0, 0, 0.35));
    transition: fill var(--duration-fast) var(--ease-standard);
  }

  .dial:hover .face,
  .dragging .face {
    fill: var(--color-surface-raised);
  }

  .pointer {
    stroke: var(--color-text);
    stroke-width: 2.5;
    stroke-linecap: round;
  }

  .label {
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
    text-align: center;
    line-height: 1.2;
  }

  .value,
  .field {
    width: calc(var(--size) + 24px);
    padding: 1px 4px;
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    text-align: center;
    color: var(--color-text);
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
  }

  .value {
    cursor: text;
  }

  .value:hover {
    border-color: var(--color-text-muted);
  }

  .field {
    outline: var(--focus-ring) solid var(--color-focus);
  }

  .disabled {
    opacity: 0.45;
  }

  .disabled .dial {
    cursor: default;
  }
</style>
