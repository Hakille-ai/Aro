import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import PreviewOverlays from "./PreviewOverlays.svelte";

function renderPreview(overrides: Record<string, unknown> = {}) {
  const onCloseFile = vi.fn();
  const onCloseCode = vi.fn();
  const result = render(PreviewOverlays, {
    file: null,
    codeOpen: false,
    codeHtml: "",
    loading: false,
    language: "en",
    theme: "light",
    onCloseFile,
    onCloseCode,
    ...overrides,
  });
  return { ...result, onCloseFile, onCloseCode };
}

describe("PreviewOverlays", () => {
  it("renders image, PDF, text and unsupported file branches", async () => {
    const { container, rerender } = renderPreview({
      file: { name: "diagram.png", mimeType: "image/png", url: "blob:image" },
    });
    expect(screen.getByRole("img", { name: "diagram.png" })).toHaveAttribute("src", "blob:image");

    await rerender({ file: { name: "report.pdf", mimeType: "application/pdf", url: "blob:pdf" } });
    expect(container.querySelector("iframe.preview-pdf-iframe")).toHaveAttribute("src", "blob:pdf");

    await rerender({ file: { name: "notes.txt", mimeType: "text/plain", url: "blob:text", content: "hello" } });
    expect(screen.getByText("hello")).toBeVisible();

    await rerender({ file: { name: "archive.zip", mimeType: "application/zip", url: "blob:zip" } });
    expect(screen.getByRole("link", { name: "Download" })).toHaveAttribute("download", "archive.zip");
  });

  it("closes on the overlay or close button but not inside the modal", async () => {
    const { container, onCloseFile } = renderPreview({
      file: { name: "notes.txt", mimeType: "text/plain", url: "blob:text", content: "hello" },
    });
    await fireEvent.click(container.querySelector(".preview-modal") as HTMLElement);
    expect(onCloseFile).not.toHaveBeenCalled();

    await fireEvent.click(container.querySelector(".preview-modal-close-btn") as HTMLElement);
    expect(onCloseFile).toHaveBeenCalledOnce();
    await fireEvent.click(container.querySelector(".preview-modal-overlay") as HTMLElement);
    expect(onCloseFile).toHaveBeenCalledTimes(2);
  });

  it("builds a sandboxed code preview and exposes loading and close states", async () => {
    const { container, onCloseCode } = renderPreview({
      codeOpen: true,
      codeHtml: "<button>Run</button>",
      loading: true,
      theme: "dark",
    });
    const frame = screen.getByTitle("Interactive Render");
    expect(frame).toHaveAttribute("sandbox", "allow-scripts");
    expect(frame.getAttribute("srcdoc")).toContain("Content-Security-Policy");
    expect(frame.getAttribute("srcdoc")).toContain("<button>Run</button>");
    expect(screen.getByText("Loading preview...")).toBeVisible();
    expect(container.querySelector(".preview-modal-body")).toHaveStyle({ background: "#1c1c1e" });

    await fireEvent.click(container.querySelector(".code-preview-modal-overlay") as HTMLElement);
    expect(onCloseCode).toHaveBeenCalledOnce();
  });
});
