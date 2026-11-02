// Everything the UI knows about the engine, in one reactive object: events go in through
// `apply`, the views read the fields, user actions become commands.
import type { Engine } from './engine';
import type { AppSource } from './types/AppSource';
import type { AudioDevice } from './types/AudioDevice';
import type { ConnectionState } from './types/ConnectionState';
import type { DeviceSlot } from './types/DeviceSlot';
import type { Event } from './types/Event';
import type { LogEntry } from './types/LogEntry';
import type { Participant } from './types/Participant';
import type { PeerLevel } from './types/PeerLevel';
import type { PersonMix } from './types/PersonMix';
import type { Settings } from './types/Settings';
import type { Sound } from './types/Sound';
import type { StageLevel } from './types/StageLevel';
import type { TrackKind } from './types/TrackKind';
import type { TrackMix } from './types/TrackMix';
import type { TransmitMode } from './types/TransmitMode';
import type { Stats } from './types/Stats';

const INVITE_KEY = 'yappa.invite';
/** Entries kept for the timeline in the debug panel. */
export const LOG_LIMIT = 300;
/** Floor of the level meters, dBFS. */
export const METER_FLOOR_DB = -60;

/** 0..1 for a meter bar. */
export function meterFill(db: number): number {
  return Math.min(1, Math.max(0, 1 - db / METER_FLOOR_DB));
}

const isDefaultMix = (track: TrackMix) => track.gainDb === 0 && !track.muted;

const inviteProblems: [RegExp, string][] = [
  [/not a yAPPA invite/, 'Das ist keine yAPPA-Einladung. Sie beginnt mit „yappa1.“'],
  [/damaged|incompletely/, 'Die Einladung ist unvollständig – bitte noch einmal ganz kopieren.'],
  [/server address/, 'Die Einladung enthält keine gültige Serveradresse.'],
];

export class Session {
  connection = $state<ConnectionState>('disconnected');
  room = $state<string | null>(null);
  identity = $state<string | null>(null);
  participants = $state<Participant[]>([]);
  inputDb = $state(-120);
  /** False while our microphone is muted. */
  sending = $state(true);
  peerLevels = $state<Record<string, PeerLevel>>({});
  stats = $state<Stats | null>(null);
  /** The send chain stage by stage (pro view). */
  stages = $state<StageLevel[]>([]);
  log = $state<LogEntry[]>([]);
  inputs = $state<AudioDevice[]>([]);
  outputs = $state<AudioDevice[]>([]);
  /** What is actually open on each side (A2): the chosen device, or a stand-in. */
  inputState = $state<DeviceSlot | null>(null);
  outputState = $state<DeviceSlot | null>(null);
  settings = $state<Settings | null>(null);
  /**
   * Microphone off and hearing off, as the engine reports them (`muteState`): the dock
   * button and the global key (A3) both end there, so this never guesses. `muted` is what is
   * in effect — true while deafened too.
   */
  muted = $state(false);
  deafened = $state(false);
  /** „Mich selbst hören“ (A5) as the engine has it (`monitorState`). */
  monitor = $state(false);
  /** Per person, as set here: volume in dB and local mute. The app settings remember the
   * whole mix (`mixes`), so it survives a restart. */
  peerGainDb = $state<Record<string, number>>({});
  peerMuted = $state<Record<string, boolean>>({});
  /** The same for their soundboard track (ADR-010). */
  peerSoundGainDb = $state<Record<string, number>>({});
  peerSoundMuted = $state<Record<string, boolean>>({});
  /** And for their app audio, the `music` track (ADR-011). */
  peerMusicGainDb = $state<Record<string, number>>({});
  peerMusicMuted = $state<Record<string, boolean>>({});
  /**
   * App audio (ADR-011): whether this OS can capture an application at all (false hides
   * the feature), the applications with a sound session as last listed, and my stream.
   */
  appsAvailable = $state(false);
  apps = $state<AppSource[]>([]);
  appAudio = $state<{ running: boolean; app: string | null }>({ running: false, app: null });
  /** The soundboard (ADR-010): the library, what plays, and why a file was refused. */
  sounds = $state<Sound[]>([]);
  playing = $state<string[]>([]);
  soundProblem = $state<string | null>(null);
  /** The last problem the engine reported, until dismissed. */
  error = $state<string | null>(null);
  invite = $state('');
  inviteProblem = $state<string | null>(null);

  readonly #engine: Engine;

  constructor(engine: Engine) {
    this.#engine = engine;
    try {
      this.invite = localStorage.getItem(INVITE_KEY) ?? '';
    } catch {
      // No storage (private mode): the invite is simply not remembered.
    }
  }

  /** Starts listening. Returns the function that stops it. */
  async start(): Promise<() => void> {
    const stop = await this.#engine.subscribe((event) => this.apply(event));
    this.settings = await this.#engine.settings();
    const earlier = await this.#engine.recentLogs();
    this.log = [...earlier, ...this.log].slice(-LOG_LIMIT);
    return stop;
  }

  apply(event: Event) {
    switch (event.type) {
      case 'connection':
        this.connection = event.state;
        if (event.state === 'disconnected') {
          this.room = null;
          this.identity = null;
          this.peerLevels = {};
        }
        break;
      case 'joined':
        this.room = event.room;
        this.identity = event.identity;
        this.error = null;
        break;
      case 'participants':
        this.participants = event.participants;
        break;
      case 'levels':
        this.inputDb = event.inputDb;
        this.sending = event.gateOpen;
        this.peerLevels = Object.fromEntries(event.peers.map((peer) => [peer.identity, peer]));
        break;
      case 'stats':
        this.stats = event.stats;
        break;
      case 'stageLevels':
        this.stages = event.stages;
        break;
      case 'devices':
        this.inputs = event.inputs;
        this.outputs = event.outputs;
        break;
      case 'deviceState':
        this.inputState = event.input;
        this.outputState = event.output;
        break;
      case 'muteState':
        this.muted = event.muted;
        this.deafened = event.deafened;
        break;
      case 'monitorState':
        this.monitor = event.on;
        break;
      case 'sounds':
        this.playing = event.playing;
        break;
      case 'apps':
        this.appsAvailable = event.available;
        this.apps = event.apps;
        break;
      case 'appAudio':
        this.appAudio = { running: event.running, app: event.app };
        break;
      case 'log':
        this.log = [...this.log, event.entry].slice(-LOG_LIMIT);
        break;
      case 'error':
        this.error = event.message;
        break;
      case 'ready':
        break;
    }
  }

  get inRoom(): boolean {
    return this.connection !== 'disconnected';
  }

  /**
   * One German line about a side that runs on a stand-in (A2) — for the banner and the
   * settings; `null` while the chosen device is open.
   */
  static fallbackNote(side: 'Mikrofon' | 'Ausgabe', slot: DeviceSlot | null): string | null {
    if (!slot?.fallback) return null;
    const chosen = slot.missing ? `${side} „${slot.missing}“` : `Dein gewähltes ${side}-Gerät`;
    const standIn = slot.open
      ? `solange läuft „${slot.open}“`
      : side === 'Mikrofon'
        ? 'und kein anderes Mikrofon ist da'
        : 'und keine andere Ausgabe ist da';
    return `${chosen} ist gerade nicht angeschlossen – ${standIn}. Es kommt von selbst zurück, sobald es wieder da ist.`;
  }

  /** The device banner text, the microphone first; `null` when both sides are as chosen. */
  get deviceNote(): string | null {
    return (
      Session.fallbackNote('Mikrofon', this.inputState) ??
      Session.fallbackNote('Ausgabe', this.outputState)
    );
  }

  async join() {
    this.inviteProblem = null;
    let invite;
    try {
      invite = await this.#engine.decodeInvite(this.invite);
    } catch (reason) {
      const text = String(reason);
      this.inviteProblem =
        inviteProblems.find(([pattern]) => pattern.test(text))?.[1] ??
        'Die Einladung ist ungültig.';
      return;
    }
    try {
      localStorage.setItem(INVITE_KEY, this.invite.trim());
    } catch {
      // see constructor
    }
    this.error = null;
    await this.#engine.send({ type: 'join', url: invite.url, token: invite.token });
  }

  leave() {
    return this.#engine.send({ type: 'leave' });
  }

  /** Flips the microphone; the engine answers with `muteState`. */
  toggleMute() {
    return this.#engine.send({ type: 'toggleMuted' });
  }

  /** Flips hearing (and, implied, the microphone); the engine answers with `muteState`. */
  toggleDeafen() {
    return this.#engine.send({ type: 'toggleDeafened' });
  }

  /** `null` = system default. */
  setInput(id: string | null) {
    if (this.settings) this.settings.inputDevice = id;
    return this.#engine.send({ type: 'setInputDevice', id });
  }

  setOutput(id: string | null) {
    if (this.settings) this.settings.outputDevice = id;
    return this.#engine.send({ type: 'setOutputDevice', id });
  }

  /** Changes some settings; the engine gets the whole object (both views write the same). */
  applySettings(patch: Partial<Settings>) {
    if (!this.settings) return Promise.resolve();
    Object.assign(this.settings, patch);
    return this.#engine.send({ type: 'applySettings', settings: $state.snapshot(this.settings) });
  }

  setTransmitMode(mode: TransmitMode) {
    if (this.settings) this.settings.transmitMode = mode;
    return this.#engine.send({ type: 'setTransmitMode', mode });
  }

  refreshDevices() {
    return this.#engine.send({ type: 'refreshDevices' });
  }

  /** „Mich selbst hören“ on or off; the engine answers with `monitorState`. */
  setMonitor(on: boolean) {
    return this.#engine.send({ type: 'setMonitor', on });
  }

  setPeerGain(identity: string, gainDb: number) {
    this.peerGainDb[identity] = gainDb;
    return this.#engine.send({ type: 'setPeerVolume', identity, track: 'voice', gainDb });
  }

  togglePeerMute(identity: string) {
    const muted = !this.peerMuted[identity];
    this.peerMuted[identity] = muted;
    return this.#engine.send({ type: 'setPeerMuted', identity, track: 'voice', muted });
  }

  /** How a person sounds for me on all three tracks. */
  mixOf(identity: string): PersonMix {
    return {
      voice: { gainDb: this.peerGainDb[identity] ?? 0, muted: this.peerMuted[identity] ?? false },
      soundboard: {
        gainDb: this.peerSoundGainDb[identity] ?? 0,
        muted: this.peerSoundMuted[identity] ?? false,
      },
      music: {
        gainDb: this.peerMusicGainDb[identity] ?? 0,
        muted: this.peerMusicMuted[identity] ?? false,
      },
    };
  }

  /** Everybody whose mix is not the default — what the app settings keep across a restart. */
  get mixes(): Record<string, PersonMix> {
    const identities = new Set([
      ...Object.keys(this.peerGainDb),
      ...Object.keys(this.peerMuted),
      ...Object.keys(this.peerSoundGainDb),
      ...Object.keys(this.peerSoundMuted),
      ...Object.keys(this.peerMusicGainDb),
      ...Object.keys(this.peerMusicMuted),
    ]);
    const mixes: Record<string, PersonMix> = {};
    for (const identity of [...identities].sort()) {
      const mix = this.mixOf(identity);
      if (!isDefaultMix(mix.voice) || !isDefaultMix(mix.soundboard) || !isDefaultMix(mix.music))
        mixes[identity] = mix;
    }
    return mixes;
  }

  /**
   * A mix the app settings remembered: into the fields the views read, and to the engine
   * for every track that is not at its default (it keeps a mix per identity for the
   * person's next visit, so once per run is enough).
   */
  async restoreMix(identity: string, mix: PersonMix) {
    const tracks: [TrackKind, TrackMix, Record<string, number>, Record<string, boolean>][] = [
      ['voice', mix.voice, this.peerGainDb, this.peerMuted],
      ['soundboard', mix.soundboard, this.peerSoundGainDb, this.peerSoundMuted],
      ['music', mix.music, this.peerMusicGainDb, this.peerMusicMuted],
    ];
    for (const [track, wanted, gains, mutes] of tracks) {
      if (isDefaultMix(wanted)) continue;
      gains[identity] = wanted.gainDb;
      mutes[identity] = wanted.muted;
      if (wanted.gainDb !== 0)
        await this.#engine.send({ type: 'setPeerVolume', identity, track, gainDb: wanted.gainDb });
      if (wanted.muted)
        await this.#engine.send({ type: 'setPeerMuted', identity, track, muted: true });
    }
  }

  exportLogs(): Promise<string> {
    return this.#engine.exportLogs();
  }

  // ── soundboard (ADR-010) ──────────────────────────────────────────────────────────────

  /** Reads the sound folder again. */
  async loadSounds() {
    this.sounds = await this.#engine.sounds();
  }

  /** Plays a sound for everybody in the channel, and for me, at its own volume; the engine
   * answers with `sounds`. */
  playSound(path: string) {
    const gainDb = this.sounds.find((s) => s.path === path)?.gainDb ?? 0;
    return this.#engine.send({ type: 'playSound', path, gainDb });
  }

  /** A sound's own volume: the engine follows at once (also while it plays), the folder
   * remembers it on `persist` (the slider's release — not every pixel of the move). */
  setSoundGain(path: string, gainDb: number, persist = false) {
    const sound = this.sounds.find((s) => s.path === path);
    if (sound) sound.gainDb = gainDb;
    this.#engine.send({ type: 'setSoundGain', path, gainDb });
    if (!persist) return Promise.resolve();
    return this.#engine.setSoundGain(path, gainDb).catch((reason) => {
      this.soundProblem = String((reason as { message?: string })?.message ?? reason);
    });
  }

  /** Moves a sound to the recycle bin and reads the folder again. */
  async removeSound(path: string) {
    this.soundProblem = null;
    try {
      await this.#engine.removeSound(path);
      await this.loadSounds();
    } catch (reason) {
      this.soundProblem = String((reason as { message?: string })?.message ?? reason);
    }
  }

  /** Their soundboard track carries a sound right now (ADR-010). */
  peerPlaysSound(identity: string): boolean {
    return this.peerLevels[identity]?.sound ?? false;
  }

  stopSounds() {
    return this.#engine.send({ type: 'stopSounds' });
  }

  isPlaying(path: string): boolean {
    return this.playing.includes(path);
  }

  /** Copies a picked file into the folder; a refusal lands in `soundProblem`. */
  async addSound(file: File) {
    this.soundProblem = null;
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      await this.#engine.addSound(file.name, bytes);
      await this.loadSounds();
    } catch (reason) {
      this.soundProblem = String((reason as { message?: string })?.message ?? reason);
    }
  }

  openSoundFolder() {
    return this.#engine.openSoundFolder();
  }

  setPeerSoundGain(identity: string, gainDb: number) {
    this.peerSoundGainDb[identity] = gainDb;
    return this.#engine.send({ type: 'setPeerVolume', identity, track: 'soundboard', gainDb });
  }

  togglePeerSoundMute(identity: string) {
    const muted = !this.peerSoundMuted[identity];
    this.peerSoundMuted[identity] = muted;
    return this.#engine.send({ type: 'setPeerMuted', identity, track: 'soundboard', muted });
  }

  // ── app audio (ADR-011) ──────────────────────────────────────────────────────────────

  /** Asks the engine for the applications with a sound session; it answers with `apps`. */
  listApps() {
    return this.#engine.send({ type: 'listApps' });
  }

  /** Streams one application into the channel; the engine answers with `appAudio`. */
  startAppAudio(app: AppSource) {
    return this.#engine.send({ type: 'startAppAudio', pid: app.pid, name: app.name });
  }

  stopAppAudio() {
    return this.#engine.send({ type: 'stopAppAudio' });
  }

  /** Their `music` track carries app audio right now (ADR-011). */
  peerStreams(identity: string): boolean {
    return this.peerLevels[identity]?.music ?? false;
  }

  setPeerMusicGain(identity: string, gainDb: number) {
    this.peerMusicGainDb[identity] = gainDb;
    return this.#engine.send({ type: 'setPeerVolume', identity, track: 'music', gainDb });
  }

  togglePeerMusicMute(identity: string) {
    const muted = !this.peerMusicMuted[identity];
    this.peerMusicMuted[identity] = muted;
    return this.#engine.send({ type: 'setPeerMuted', identity, track: 'music', muted });
  }
}
