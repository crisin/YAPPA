// The rack knob: a slider to the accessibility tree, a dial to the eye. Keyboard, wheel,
// double-click and the typed value all go through `onchange`; the knob never changes its
// own prop.
import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import Knob from './Knob.svelte';
import type { ComponentProps } from 'svelte';
import { axe } from '../../test-a11y';

function knob(props: Partial<ComponentProps<typeof Knob>> = {}) {
  const onchange = vi.fn();
  const result = render(Knob, {
    props: {
      id: 'k',
      label: 'Schwelle',
      value: -18,
      min: -40,
      max: 0,
      step: 1,
      defaultValue: -18,
      format: (v: number) => `${v} dB`,
      onchange,
      ...props,
    },
  });
  const label = props.label ?? 'Schwelle';
  return { ...result, onchange, dial: screen.getByRole('slider', { name: label }) };
}

describe('Knob', () => {
  it('is a slider with the formatted value as text', async () => {
    const { dial, container } = knob();
    expect(dial).toHaveAttribute('aria-valuemin', '-40');
    expect(dial).toHaveAttribute('aria-valuemax', '0');
    expect(dial).toHaveAttribute('aria-valuenow', '-18');
    expect(dial).toHaveAttribute('aria-valuetext', '-18 dB');
    expect(
      screen.getByRole('button', { name: 'Schwelle: -18 dB – Wert eingeben' }),
    ).toBeInTheDocument();
    expect(await axe(container)).toHaveNoViolations();
  });

  it('steps with the arrow keys, ten steps with Shift and Page keys, stops at the ends', async () => {
    const { dial, onchange } = knob();
    await fireEvent.keyDown(dial, { key: 'ArrowUp' });
    expect(onchange).toHaveBeenLastCalledWith(-17);
    await fireEvent.keyDown(dial, { key: 'ArrowLeft' });
    expect(onchange).toHaveBeenLastCalledWith(-19);
    await fireEvent.keyDown(dial, { key: 'ArrowUp', shiftKey: true });
    expect(onchange).toHaveBeenLastCalledWith(-8);
    await fireEvent.keyDown(dial, { key: 'PageDown' });
    expect(onchange).toHaveBeenLastCalledWith(-28);
    await fireEvent.keyDown(dial, { key: 'End' });
    expect(onchange).toHaveBeenLastCalledWith(0);
    await fireEvent.keyDown(dial, { key: 'Home' });
    expect(onchange).toHaveBeenLastCalledWith(-40);
    onchange.mockClear();
    // Already at -18 and the default is -18: nothing to send.
    await fireEvent.dblClick(dial);
    expect(onchange).not.toHaveBeenCalled();
  });

  it('quantizes to the step and clamps to the stops', async () => {
    const { dial, onchange } = knob({ value: -1, step: 0.5 });
    await fireEvent.keyDown(dial, { key: 'ArrowUp' });
    expect(onchange).toHaveBeenLastCalledWith(-0.5);
    await fireEvent.keyDown(dial, { key: 'ArrowUp', shiftKey: true });
    expect(onchange).toHaveBeenLastCalledWith(0);
  });

  it('goes back to the default on double-click', async () => {
    const { dial, onchange } = knob({ value: -30 });
    await fireEvent.dblClick(dial);
    expect(onchange).toHaveBeenLastCalledWith(-18);
  });

  it('turns with the wheel without scrolling the page', async () => {
    const { dial, onchange } = knob();
    const up = new WheelEvent('wheel', { deltaY: -100, cancelable: true });
    dial.dispatchEvent(up);
    expect(up.defaultPrevented).toBe(true);
    expect(onchange).toHaveBeenLastCalledWith(-17);
    dial.dispatchEvent(new WheelEvent('wheel', { deltaY: 100, cancelable: true }));
    expect(onchange).toHaveBeenLastCalledWith(-19);
  });

  it('takes a typed value with a comma, Enter commits, Escape drops it', async () => {
    const { onchange } = knob({ step: 0.5 });
    await fireEvent.click(screen.getByRole('button', { name: /Wert eingeben/ }));
    const field = screen.getByRole('textbox', { name: 'Schwelle eingeben' });
    await fireEvent.input(field, { target: { value: '-12,5' } });
    await fireEvent.keyDown(field, { key: 'Enter' });
    expect(onchange).toHaveBeenLastCalledWith(-12.5);
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();

    onchange.mockClear();
    await fireEvent.click(screen.getByRole('button', { name: /Wert eingeben/ }));
    const again = screen.getByRole('textbox', { name: 'Schwelle eingeben' });
    await fireEvent.input(again, { target: { value: 'abc' } });
    await fireEvent.keyDown(again, { key: 'Escape' });
    expect(onchange).not.toHaveBeenCalled();
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
  });

  it('walks a log taper in ratios: a notch at 1 kHz is bigger than at 100 Hz', async () => {
    const { dial, onchange } = knob({
      label: 'Frequenz',
      value: 1000,
      min: 20,
      max: 20000,
      log: true,
      defaultValue: 1000,
    });
    await fireEvent.keyDown(dial, { key: 'ArrowUp' });
    const at1k = onchange.mock.lastCall?.[0] as number;
    expect(at1k).toBeGreaterThan(1000);
    expect(at1k).toBeLessThan(1200);
  });

  it('does nothing while disabled', async () => {
    const { dial, onchange } = knob({ disabled: true });
    expect(dial).toHaveAttribute('aria-disabled', 'true');
    expect(dial).toHaveAttribute('tabindex', '-1');
    await fireEvent.keyDown(dial, { key: 'ArrowUp' });
    expect(onchange).not.toHaveBeenCalled();
  });
});
