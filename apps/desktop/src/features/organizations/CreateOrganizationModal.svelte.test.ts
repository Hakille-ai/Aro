import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import CreateOrganizationModal from "./CreateOrganizationModal.svelte";

function renderModal(overrides: Record<string, unknown> = {}) {
  const onClose = vi.fn();
  const onCreate = vi.fn().mockResolvedValue(undefined);
  const result = render(CreateOrganizationModal, {
    language: "en",
    name: "",
    busy: false,
    error: "",
    onClose,
    onCreate,
    ...overrides,
  });
  return { ...result, onClose, onCreate };
}

describe("CreateOrganizationModal", () => {
  it("binds the workspace name and creates then closes", async () => {
    const { onClose, onCreate } = renderModal();
    const submit = screen.getByRole("button", { name: /confirm creation/i });
    expect(submit).toBeDisabled();

    await fireEvent.input(screen.getByPlaceholderText("Workspace / Organization name"), {
      target: { value: "Research" },
    });
    expect(submit).toBeEnabled();
    await fireEvent.click(submit);

    expect(onCreate).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalledOnce();
    expect(onCreate.mock.invocationCallOrder[0]).toBeLessThan(onClose.mock.invocationCallOrder[0]);
  });

  it("exposes close, error, busy and language branches", async () => {
    const { onClose } = renderModal({ language: "fr", busy: true, error: "Workspace unavailable" });

    expect(screen.getByText("Workspace unavailable")).toBeVisible();
    expect(screen.getByPlaceholderText(/Workspace/)).toBeDisabled();
    expect(screen.getByRole("button", { name: /Confirmer/i })).toBeDisabled();
    await fireEvent.click(screen.getByRole("button", { name: "Fermer" }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});
