import { describe, expect, it } from 'vitest';
import { DEFAULT_SCENE, SCENES, isScene, sceneFromUrl } from './scene';

describe('scene', () => {
  it('reads the scene from the query string', () => {
    expect(sceneFromUrl('?scene=reconnect')).toBe('reconnect');
    expect(sceneFromUrl('?foo=1&scene=offline')).toBe('offline');
  });

  it('falls back to the default for nothing or nonsense', () => {
    expect(sceneFromUrl('')).toBe(DEFAULT_SCENE);
    expect(sceneFromUrl('?scene=matrix')).toBe(DEFAULT_SCENE);
    expect(isScene('toString')).toBe(false);
  });

  it('describes every scene in one sentence', () => {
    for (const text of Object.values(SCENES)) expect(text.length).toBeGreaterThan(10);
  });
});
