// Voice quality profiles as the UI shows them. The numbers come from engine-protocol
// (`cargo xtask gen-types` writes qualityProfiles.json), the German words live here.
import type { OpusFixed } from './types/OpusFixed';
import type { QualityProfile } from './types/QualityProfile';
import type { Settings } from './types/Settings';
import type { VoiceCodec } from './types/VoiceCodec';
import generated from './types/qualityProfiles.json';

interface Generated {
  profiles: { id: QualityProfile; codec: VoiceCodec }[];
  fixed: OpusFixed;
}

const data = generated as Generated;

/** Frame length, FEC and complexity: decided inside libwebrtc, shown as facts. */
export const OPUS_FIXED: OpusFixed = data.fixed;

export const PROFILE_LABEL: Record<QualityProfile, string> = {
  thrifty: 'Sparsam',
  standard: 'Standard',
  high: 'Hoch',
  studio: 'Studio',
  stress: 'Stress',
  custom: 'Eigen',
};

export const PROFILE_HINT: Record<QualityProfile, string> = {
  thrifty: 'Für einen schwachen Upload. Sprache bleibt klar, Musik im Hintergrund wird matschig.',
  standard: 'Der bewährte Wert der ersten Abende – klingt wie Discord, kostet wenig.',
  high: 'Mehr Reserve für Stimme und Raum. Der Unterschied ist klein, aber hörbar.',
  studio: 'Die höchste Stufe, bei der die Redundanz (RED) noch ins Paket passt.',
  stress:
    'Das Maximum von Opus angefordert, Stereo, ohne Redundanz; libwebrtc deckelt bei etwa 256 kbit/s – die Diagnose zeigt, was ankommt.',
  custom: 'Deine Werte aus „Netzwerk“ (Profi).',
};

/** The named profiles in the order of the picker, with their values. */
export const PROFILES: { id: QualityProfile; codec: VoiceCodec }[] = data.profiles;

export function presetOf(profile: QualityProfile): VoiceCodec | null {
  return PROFILES.find((p) => p.id === profile)?.codec ?? null;
}

/** What is actually sent: the profile's preset, or the custom values. */
export function codecOf(settings: Pick<Settings, 'quality' | 'voice'>): VoiceCodec {
  return presetOf(settings.quality) ?? settings.voice;
}

/** The named profile with exactly these values, else `custom` — mirrors `QualityProfile::matching`. */
export function matching(codec: VoiceCodec): QualityProfile {
  return (
    PROFILES.find(
      (p) =>
        p.codec.bitrateKbps === codec.bitrateKbps &&
        p.codec.stereo === codec.stereo &&
        p.codec.red === codec.red &&
        p.codec.dtx === codec.dtx,
    )?.id ?? 'custom'
  );
}

/** One line for a codec: „80 kbit/s · Mono · RED“. */
export function describe(codec: VoiceCodec): string {
  const parts = [`${codec.bitrateKbps} kbit/s`, codec.stereo ? 'Stereo' : 'Mono'];
  if (codec.red) parts.push('RED');
  if (codec.dtx) parts.push('DTX');
  return parts.join(' · ');
}
