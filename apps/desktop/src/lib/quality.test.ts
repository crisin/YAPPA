import { describe as suite, expect, it } from 'vitest';
import {
  OPUS_FIXED,
  PROFILES,
  PROFILE_LABEL,
  codecOf,
  describe,
  matching,
  presetOf,
} from './quality';
import defaultSettings from './types/defaultSettings.json';
import type { Settings } from './types/Settings';

const defaults = defaultSettings as Settings;

suite('quality profiles', () => {
  it('lists the five named profiles from engine-protocol, each with a German label', () => {
    expect(PROFILES.map((p) => p.id)).toEqual(['thrifty', 'standard', 'high', 'studio', 'stress']);
    for (const p of PROFILES) expect(PROFILE_LABEL[p.id]).toBeTruthy();
    expect(PROFILE_LABEL.custom).toBe('Eigen');
  });

  it('sends the preset for a named profile and the own values for custom', () => {
    expect(defaults.quality).toBe('standard');
    expect(codecOf(defaults).bitrateKbps).toBe(80);
    const custom: Settings = {
      ...defaults,
      quality: 'custom',
      voice: { bitrateKbps: 200, stereo: true, red: false, dtx: true },
    };
    expect(codecOf(custom)).toEqual(custom.voice);
    expect(codecOf({ ...custom, quality: 'stress' })).toEqual(presetOf('stress'));
  });

  it('finds the profile again from its values, like the Rust side', () => {
    for (const p of PROFILES) expect(matching(p.codec)).toBe(p.id);
    expect(matching({ bitrateKbps: 80, stereo: false, red: false, dtx: false })).toBe('custom');
  });

  it('describes a codec in one line and knows what libwebrtc fixes', () => {
    expect(describe(presetOf('standard')!)).toBe('80 kbit/s · Mono · RED');
    expect(describe(presetOf('stress')!)).toBe('510 kbit/s · Stereo');
    expect(describe(presetOf('studio')!)).toBe('192 kbit/s · Mono · RED');
    expect(OPUS_FIXED).toEqual({ frameMs: 20, inbandFec: true, complexity: 9 });
  });
});
