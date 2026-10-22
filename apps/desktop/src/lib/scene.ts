// Mock scenes: what the browser mode (`cargo xtask dev --ui --scene <name>`) and the tests
// show without a server and without audio. Both mocks — engine and API — read the scene,
// so one word puts the whole UI into a state that is otherwise hard to reach on demand.
export const SCENES = {
  call: 'Im Call: Raid-Abend mit fünf Leuten, Krille (du) ist Admin',
  lobby: 'Allein in der Lobby, sonst ist nur Rico da (abwesend)',
  member: 'Wie Ben es sieht: Mitglied ohne Admin-Rolle',
  reconnect: 'Die Verbindung bricht alle paar Sekunden weg und kommt wieder',
  offline: 'Weder Control-Plane noch LiveKit sind erreichbar',
  update: 'Der Server verlangt eine neuere Mindestversion',
} as const;

export type Scene = keyof typeof SCENES;

export const DEFAULT_SCENE: Scene = 'call';

export function isScene(name: string | null | undefined): name is Scene {
  return name != null && Object.hasOwn(SCENES, name);
}

/** `?scene=<name>` from the page URL; anything unknown is the default scene. */
export function sceneFromUrl(search: string = globalThis.location?.search ?? ''): Scene {
  const name = new URLSearchParams(search).get('scene');
  return isScene(name) ? name : DEFAULT_SCENE;
}
