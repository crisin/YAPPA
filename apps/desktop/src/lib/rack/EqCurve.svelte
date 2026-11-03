<script lang="ts">
  // The EQ's frequency response, drawn from the same peaking-biquad formula the engine uses
  // (RBJ cookbook, 48 kHz). One handle per band: drag it left/right for the frequency,
  // up/down for the gain. The handles double the knobs underneath, so they stay out of the
  // accessibility tree; the curve itself is an image with a text alternative.
  import type { EqBand } from '../types/EqBand';

  let {
    bands,
    onchange,
  }: { bands: EqBand[]; onchange: (index: number, patch: Partial<EqBand>) => void } = $props();

  const W = 320;
  const H = 120;
  const PAD = 10;
  const F_MIN = 20;
  const F_MAX = 20_000;
  const DB_MAX = 15;
  const SR = 48_000;

  const x = (f: number) =>
    PAD + ((Math.log(f) - Math.log(F_MIN)) / (Math.log(F_MAX) - Math.log(F_MIN))) * (W - 2 * PAD);
  const y = (db: number) =>
    H / 2 - (Math.max(-DB_MAX, Math.min(DB_MAX, db)) / DB_MAX) * (H / 2 - PAD);
  const fromX = (px: number) =>
    Math.exp(
      Math.log(F_MIN) +
        Math.min(1, Math.max(0, (px - PAD) / (W - 2 * PAD))) * (Math.log(F_MAX) - Math.log(F_MIN)),
    );
  const fromY = (py: number) => (-(py - H / 2) / (H / 2 - PAD)) * DB_MAX;

  /** |H| in dB of one peaking band at frequency `f`. */
  function bandDb(b: EqBand, f: number): number {
    const a = Math.pow(10, b.gainDb / 40);
    const w0 = (2 * Math.PI * Math.min(20_000, Math.max(20, b.freqHz))) / SR;
    const alpha = Math.sin(w0) / (2 * Math.max(0.1, b.q));
    const cos0 = Math.cos(w0);
    const b0 = 1 + alpha * a;
    const b1 = -2 * cos0;
    const b2 = 1 - alpha * a;
    const a0 = 1 + alpha / a;
    const a1 = -2 * cos0;
    const a2 = 1 - alpha / a;
    const w = (2 * Math.PI * f) / SR;
    const c1 = Math.cos(w);
    const c2 = Math.cos(2 * w);
    const s1 = Math.sin(w);
    const s2 = Math.sin(2 * w);
    const num = (b0 + b1 * c1 + b2 * c2) ** 2 + (b1 * s1 + b2 * s2) ** 2;
    const den = (a0 + a1 * c1 + a2 * c2) ** 2 + (a1 * s1 + a2 * s2) ** 2;
    return 10 * Math.log10(num / den);
  }

  const POINTS = 96;
  const path = $derived.by(() => {
    const parts: string[] = [];
    for (let i = 0; i <= POINTS; i++) {
      const f = fromX(PAD + (i / POINTS) * (W - 2 * PAD));
      const db = bands.reduce((sum, b) => sum + bandDb(b, f), 0);
      parts.push(`${i ? 'L' : 'M'}${x(f).toFixed(1)} ${y(db).toFixed(1)}`);
    }
    return parts.join(' ');
  });

  const GRID_HZ = [100, 1000, 10_000];
  const GRID_DB = [-10, 10];

  const describe = $derived(
    bands.length === 0
      ? 'EQ-Kurve: flach'
      : `EQ-Kurve: ${bands
          .map(
            (b, i) =>
              `Band ${i + 1} ${Math.round(b.freqHz)} Hz ${b.gainDb > 0 ? '+' : ''}${b.gainDb} dB`,
          )
          .join(', ')}`,
  );

  // Handle drag in SVG units; the rectangle maps client pixels onto the viewBox.
  let svg = $state<SVGSVGElement | null>(null);
  let drag: { index: number; pointer: number } | null = null;

  function down(e: PointerEvent, index: number) {
    if (e.button !== 0) return;
    drag = { index, pointer: e.pointerId };
    (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
    e.preventDefault();
  }
  function move(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointer || !svg) return;
    const rect = svg.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return;
    const px = ((e.clientX - rect.left) / rect.width) * W;
    const py = ((e.clientY - rect.top) / rect.height) * H;
    onchange(drag.index, {
      freqHz: Math.round(fromX(px)),
      gainDb: Math.round(Math.max(-12, Math.min(12, fromY(py))) * 2) / 2,
    });
  }
  function up(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointer) return;
    (e.currentTarget as Element).releasePointerCapture?.(e.pointerId);
    drag = null;
  }
</script>

<svg
  bind:this={svg}
  class="curve"
  viewBox="0 0 {W} {H}"
  role="img"
  aria-label={describe}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
>
  {#each GRID_HZ as f (f)}
    <line class="grid" x1={x(f)} x2={x(f)} y1={PAD} y2={H - PAD} />
    <text class="tick" x={x(f) + 3} y={H - 2}>{f >= 1000 ? `${f / 1000}k` : f}</text>
  {/each}
  {#each GRID_DB as db (db)}
    <line class="grid" x1={PAD} x2={W - PAD} y1={y(db)} y2={y(db)} />
    <text class="tick" x={PAD + 2} y={y(db) - 2}>{db > 0 ? '+' : ''}{db}</text>
  {/each}
  <line class="zero" x1={PAD} x2={W - PAD} y1={y(0)} y2={y(0)} />
  <path class="response" d={path} />
  {#each bands as b, i (i)}
    <g class="handle" aria-hidden="true" onpointerdown={(e) => down(e, i)}>
      <circle cx={x(b.freqHz)} cy={y(b.gainDb)} r="9" />
      <text x={x(b.freqHz)} y={y(b.gainDb) + 3.5}>{i + 1}</text>
    </g>
  {/each}
</svg>

<style>
  .curve {
    display: block;
    width: 100%;
    max-width: 480px;
    aspect-ratio: 320 / 120;
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    touch-action: none;
  }

  .grid {
    stroke: var(--color-border);
    stroke-width: 0.5;
  }

  .zero {
    stroke: var(--color-text-muted);
    stroke-width: 0.75;
    stroke-dasharray: 2 3;
  }

  .tick {
    fill: var(--color-text-muted);
    font-size: 8px;
    font-family: var(--font-mono);
  }

  .response {
    fill: none;
    stroke: var(--color-accent);
    stroke-width: 2;
    stroke-linejoin: round;
  }

  .handle {
    cursor: move;
  }

  .handle circle {
    fill: var(--color-surface);
    stroke: var(--color-accent);
    stroke-width: 1.5;
  }

  .handle:hover circle {
    fill: var(--color-accent);
  }

  .handle text {
    fill: var(--color-text);
    font-size: 9px;
    font-weight: 600;
    text-anchor: middle;
    pointer-events: none;
  }

  .handle:hover text {
    fill: var(--color-accent-text);
  }
</style>
