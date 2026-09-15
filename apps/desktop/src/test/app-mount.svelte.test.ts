import { describe, it, expect } from 'vitest';
import App from '../App.svelte';
import { mount } from 'svelte';

describe('App mounting', () => {
  it('mounts App without throwing', () => {
    const target = document.createElement('div');
    document.body.appendChild(target);
    expect(() => {
      mount(App, { target });
    }).not.toThrow();
  });
});
