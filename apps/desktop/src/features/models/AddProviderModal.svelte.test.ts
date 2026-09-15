import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import AddProviderModal from "./AddProviderModal.svelte";

const defaults = {
  openai: { name: "OpenAI", endpoint: "https://api.openai.com/v1" },
  ollama: { name: "Ollama", endpoint: "http://127.0.0.1:11434" },
  mock: { name: "Mock", endpoint: "" },
};

const options = [
  { value: "mock", label: "Mock" },
  { value: "openai", label: "OpenAI" },
  { value: "ollama", label: "Ollama" },
];

function renderModal(overrides: Record<string, unknown> = {}) {
  const onClose = vi.fn();
  const onSubmit = vi.fn();
  const result = render(AddProviderModal, {
    language: "en",
    kind: "openai",
    name: "OpenAI",
    endpoint: defaults.openai.endpoint,
    apiKey: "",
    writeLocked: false,
    options,
    defaults,
    onClose,
    onSubmit,
    ...overrides,
  });
  return { ...result, onClose, onSubmit };
}

describe("AddProviderModal", () => {
  it("filters mock, applies provider defaults and hides local API keys", async () => {
    renderModal();
    const provider = screen.getByRole("combobox");

    expect(screen.queryByRole("option", { name: "Mock" })).not.toBeInTheDocument();
    expect(screen.getByPlaceholderText("API key saved locally")).toBeVisible();

    await fireEvent.change(provider, { target: { value: "ollama" } });
    expect(screen.getByPlaceholderText("Display name")).toHaveValue("Ollama");
    expect(screen.getByPlaceholderText("Endpoint")).toHaveValue("http://127.0.0.1:11434");
    expect(screen.queryByPlaceholderText("API key saved locally")).not.toBeInTheDocument();
  });

  it("submits valid values, closes explicitly, and enforces write lock", async () => {
    const { onClose, onSubmit, rerender } = renderModal();
    await fireEvent.submit(screen.getByRole("button", { name: "Add" }).closest("form") as HTMLFormElement);
    expect(onSubmit).toHaveBeenCalledOnce();

    await fireEvent.click(screen.getByRole("button", { name: "Fermer" }));
    expect(onClose).toHaveBeenCalledOnce();

    await rerender({ writeLocked: true });
    expect(screen.getByRole("button", { name: "Add" })).toBeDisabled();
  });
});
