import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import SplashScreen from "./SplashScreen.svelte";

describe("SplashScreen", () => {
  it("renders the brand and reflects progress and theme props", async () => {
    const { container, rerender } = render(SplashScreen, { dark: false, progress: 25 });

    expect(screen.getByRole("heading", { name: "ARO" })).toBeVisible();
    expect(screen.getByRole("img", { name: "ARO Logo" })).toHaveAttribute("src", "/logo.png");
    expect(container.querySelector(".splash-progress-bar")).toHaveStyle({ width: "25%" });
    expect(container.querySelector(".splash-screen")).not.toHaveClass("dark-theme");

    await rerender({ dark: true, progress: 100 });
    expect(container.querySelector(".splash-progress-bar")).toHaveStyle({ width: "100%" });
    expect(container.querySelector(".splash-screen")).toHaveClass("dark-theme");
  });

  it("does not bubble clicks to the application underneath", async () => {
    const onBodyClick = vi.fn();
    document.body.addEventListener("click", onBodyClick);
    const { container } = render(SplashScreen, { dark: false, progress: 0 });

    await fireEvent.click(container.querySelector(".splash-screen") as HTMLElement);
    expect(onBodyClick).not.toHaveBeenCalled();
    document.body.removeEventListener("click", onBodyClick);
  });
});
