import { beforeEach, describe, expect, it } from 'vitest';
import { get } from 'svelte/store';
import type { UpdateInfoDto } from '$lib/bindings';
import { availableUpdate, dismissUpdate, offerUpdate } from './update';
import { applyUpdateAvailable } from '$lib/ipc/router';

const info: UpdateInfoDto = {
  version: '1.2.0',
  url: 'https://github.com/timhartmann7/omnyssh/releases/tag/v1.2.0',
  tag: 'v1.2.0',
  canSelfUpdate: true
};

describe('update banner store', () => {
  beforeEach(() => dismissUpdate());

  it('starts hidden', () => {
    expect(get(availableUpdate)).toBeNull();
  });

  it('offerUpdate shows the update; dismiss hides it', () => {
    offerUpdate(info);
    expect(get(availableUpdate)).toEqual(info);
    dismissUpdate();
    expect(get(availableUpdate)).toBeNull();
  });

  it('applyUpdateAvailable routes an update-available event into the banner', () => {
    applyUpdateAvailable({ info });
    expect(get(availableUpdate)).toEqual(info);
  });
});
