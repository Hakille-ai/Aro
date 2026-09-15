import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/svelte";
import { afterEach, vi } from "vitest";

if (!Element.prototype.animate) {
  Element.prototype.animate = vi.fn(() => ({
    cancel: vi.fn(),
    finish: vi.fn(),
    finished: Promise.resolve(),
    pause: vi.fn(),
    play: vi.fn(),
    reverse: vi.fn(),
  }) as unknown as Animation);
}

afterEach(() => {
  cleanup();
  document.body.className = "";
  localStorage.clear();
});
