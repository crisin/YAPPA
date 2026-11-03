// The EQ curve: the sum of the bands' peaking responses, described in words for the
// accessibility tree; the handles are a convenience next to the knobs.
import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import EqCurve from './EqCurve.svelte';

describe('EqCurve', () => {
  it('says "flach" without bands and names each band with one', () => {
    const onchange = vi.fn();
    const { rerender } = render(EqCurve, { props: { bands: [], onchange } });
    expect(screen.getByRole('img', { name: 'EQ-Kurve: flach' })).toBeInTheDocument();
    void rerender({
      bands: [
        { freqHz: 3000, gainDb: 4, q: 1 },
        { freqHz: 120, gainDb: -3, q: 2 },
      ],
      onchange,
    });
    expect(
      screen.getByRole('img', { name: 'EQ-Kurve: Band 1 3000 Hz +4 dB, Band 2 120 Hz -3 dB' }),
    ).toBeInTheDocument();
  });

  it('draws a boost as a bump above the zero line at the band frequency', () => {
    const { container } = render(EqCurve, {
      props: { bands: [{ freqHz: 1000, gainDb: 12, q: 1 }], onchange: vi.fn() },
    });
    const d = container.querySelector('.response')?.getAttribute('d') ?? '';
    const ys = d
      .split(' ')
      .filter((_, i) => i % 2 === 1)
      .map(Number);
    const zero = 60;
    // Peak above the line (smaller y), the edges on it.
    expect(Math.min(...ys)).toBeLessThan(zero - 30);
    expect(ys[0]).toBeCloseTo(zero, 0);
    expect(ys[ys.length - 1]).toBeCloseTo(zero, 0);
  });
});
