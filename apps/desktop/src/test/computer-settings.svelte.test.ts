import { describe, it, expect, beforeEach } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";
import ComputerSettings from "../features/settings/pages/ComputerSettings.svelte";
import { computerPermissions, resetComputerPermissions } from "../lib/computer/computer-service";
import { get } from "svelte/store";

describe("ComputerSettings component", () => {
  beforeEach(() => {
    resetComputerPermissions();
  });

  it("renders ComputerSettings in French with correct headers", () => {
    const { getByText } = render(ComputerSettings, {
      props: {
        language: "fr",
        writeLocked: false,
      },
    });

    expect(getByText("Ordinateur & Système")).toBeDefined();
    expect(getByText("Contrôle Système & Autorisation Globale")).toBeDefined();
    expect(getByText("Partage du Contexte Environnement (Instructions IA)")).toBeDefined();
    expect(getByText("Outils Système Autonomes de l'IA")).toBeDefined();
    expect(getByText("Télémétrie en Direct & Actions de Test")).toBeDefined();
  });

  it("renders ComputerSettings in English with correct headers", () => {
    const { getByText } = render(ComputerSettings, {
      props: {
        language: "en",
        writeLocked: false,
      },
    });

    expect(getByText("Computer & OS Integration")).toBeDefined();
    expect(getByText("Workstation Control & Master Switch")).toBeDefined();
    expect(getByText("Environment Context Sharing in AI Prompts")).toBeDefined();
    expect(getByText("Autonomous System Tools for AI Agents")).toBeDefined();
    expect(getByText("Live Telemetry & Interactive Testing")).toBeDefined();
  });

  it("reflects master toggle switch", async () => {
    const { container } = render(ComputerSettings, {
      props: {
        language: "fr",
        writeLocked: false,
      },
    });

    expect(get(computerPermissions).enabled).toBe(true);

    const checkboxes = container.querySelectorAll<HTMLInputElement>('input[type="checkbox"]');
    expect(checkboxes.length).toBeGreaterThan(0);

    // The first checkbox is the master switch
    const masterCheckbox = checkboxes[0];
    expect(masterCheckbox.checked).toBe(true);

    await fireEvent.click(masterCheckbox);
    expect(get(computerPermissions).enabled).toBe(false);
  });

  it("allows toggling individual permissions", async () => {
    const { container } = render(ComputerSettings, {
      props: {
        language: "fr",
        writeLocked: false,
      },
    });

    // Checkbox 3 is battery (0: enabled, 1: allowComputerUse, 2: confirmDangerousActions, 3: shareBattery)
    const checkboxes = container.querySelectorAll<HTMLInputElement>('input[type="checkbox"]');
    const batteryCheckbox = checkboxes[3];
    expect(batteryCheckbox.checked).toBe(true);

    await fireEvent.click(batteryCheckbox);
    expect(get(computerPermissions).shareBattery).toBe(false);
  });
});
