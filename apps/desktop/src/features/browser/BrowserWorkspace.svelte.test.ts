import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import BrowserWorkspace from "./BrowserWorkspace.svelte";

describe("BrowserWorkspace", () => {
  it("shows the engine-off state outside Tauri without crashing", async () => {
    render(BrowserWorkspace, { props: { language: "fr" } });
    // Hors Tauri (vitest) : pas de Chromium live, état explicite attendu.
    const hits = await screen.findAllByText(/Moteur arrêté|moteur navigateur indisponible/i);
    expect(hits.length).toBeGreaterThan(0);
  });

  it("renders agent-provided event tabs with their content", async () => {
    const { container } = render(BrowserWorkspace, { props: { language: "fr" } });
    window.dispatchEvent(
      new CustomEvent("aro:agent-browser-step", {
        detail: {
          url: "https://example.com/article",
          title: "Example Article",
          stepTitle: "Browser action: extract",
          content: "Ceci est le contenu extrait de test.",
        },
      })
    );
    const titles = await screen.findAllByText("Example Article");
    expect(titles.length).toBeGreaterThan(0);
    expect(await screen.findByText(/contenu extrait de test/)).toBeTruthy();
    expect(container.querySelector(".browser-workspace")).toBeTruthy();
  });

  it("takes and releases user control", async () => {
    render(BrowserWorkspace, { props: { language: "fr" } });
    const take = await screen.findByText(/Prendre le contrôle/);
    await fireEvent.click(take);
    expect(await screen.findByText(/Vous pilotez/)).toBeTruthy();
    const release = await screen.findByText(/Rendre à l'IA/);
    await fireEvent.click(release);
    expect(await screen.findByText(/Prendre le contrôle/)).toBeTruthy();
  });
});
