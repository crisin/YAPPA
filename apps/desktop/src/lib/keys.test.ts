import { describe, expect, it } from 'vitest';
import { keyLabel } from './keys';

describe('keyLabel', () => {
  it('names keys and buttons the way people say them', () => {
    expect(keyLabel('KeyV')).toBe('V');
    expect(keyLabel('Num4')).toBe('4');
    expect(keyLabel('CapsLock')).toBe('Feststelltaste');
    expect(keyLabel('Mouse4')).toBe('Maustaste 4');
    expect(keyLabel('Unknown(124)')).toBe('Taste 124');
    expect(keyLabel('F9')).toBe('F9');
    expect(keyLabel(null)).toBe('keine');
  });
});
