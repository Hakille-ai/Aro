import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import SpreadsheetViewer from "./SpreadsheetViewer.svelte";
import WordDocumentViewer from "./WordDocumentViewer.svelte";
import MediaViewer from "./MediaViewer.svelte";
import CodeMarkdownViewer from "./CodeMarkdownViewer.svelte";
import PreviewOverlays from "./PreviewOverlays.svelte";
import AgentStepCard from "../chat/AgentStepCard.svelte";
import type { SpreadsheetData } from "../../lib/documents/excel-parser";
import type { WordDocumentData } from "../../lib/documents/word-parser";
import type { AgentStep, PreviewFile } from "../../lib/types";

describe("Document & File Visualizers", () => {
  describe("SpreadsheetViewer", () => {
    const mockData: SpreadsheetData = {
      sheets: [
        {
          name: "Ventes 2026",
          headers: ["Produit", "Quantité", "Total"],
          rows: [
            ["MacBook", 10, 24990],
            ["iPhone", 50, 59950],
            ["iPad", 20, 15980],
          ],
        },
        {
          name: "Clients",
          headers: ["Nom", "Ville"],
          rows: [["Acme Corp", "Paris"]],
        },
      ],
      activeSheetIndex: 0,
    };

    it("renders headers, rows, and allows switching sheets", async () => {
      render(SpreadsheetViewer, {
        data: mockData,
        fileName: "bilan.xlsx",
        language: "fr",
        theme: "light",
      });

      expect(screen.getByText("bilan.xlsx")).toBeInTheDocument();
      expect(screen.getByText("Produit")).toBeInTheDocument();
      expect(screen.getByText("MacBook")).toBeInTheDocument();

      // Sheet tabs
      const clientTab = screen.getByText("Clients");
      expect(clientTab).toBeInTheDocument();
      await fireEvent.click(clientTab);

      expect(screen.getByText("Acme Corp")).toBeInTheDocument();
    });

    it("filters rows using search input", async () => {
      render(SpreadsheetViewer, {
        data: mockData,
        fileName: "bilan.xlsx",
        language: "fr",
        theme: "light",
      });

      const searchInput = screen.getByPlaceholderText("Rechercher dans les cellules...");
      await fireEvent.input(searchInput, { target: { value: "iPhone" } });

      expect(screen.getByText("iPhone")).toBeInTheDocument();
      expect(screen.queryByText("MacBook")).not.toBeInTheDocument();
    });
  });

  describe("WordDocumentViewer", () => {
    const mockWordData: WordDocumentData = {
      title: "Rapport Stratégique",
      subtitle: "Exercice 2026-2027",
      sections: [
        { type: "heading", level: 1, text: "Objectifs Principaux" },
        {
          type: "paragraph",
          text: "Déploiement de l'architecture Apple-grade.",
          runs: [
            { text: "Déploiement ", bold: true },
            { text: "de l'architecture Apple-grade.", italic: true },
          ],
        },
        { type: "list-item", text: "Performance et persistance du streaming" },
        {
          type: "table",
          tableData: {
            headers: ["Jalon", "Date"],
            rows: [["Lancement", "T1 2026"]],
          },
        },
      ],
    };

    it("renders title, subtitle, headings, lists and tables", () => {
      render(WordDocumentViewer, {
        data: mockWordData,
        fileName: "rapport.docx",
        language: "fr",
        theme: "light",
      });

      expect(screen.getByText("Rapport Stratégique")).toBeInTheDocument();
      expect(screen.getByText("Exercice 2026-2027")).toBeInTheDocument();
      expect(screen.getByText("Objectifs Principaux")).toBeInTheDocument();
      expect(screen.getByText("Performance et persistance du streaming")).toBeInTheDocument();
      expect(screen.getByText("Lancement")).toBeInTheDocument();
    });

    it("handles zoom controls", async () => {
      render(WordDocumentViewer, {
        data: mockWordData,
        fileName: "rapport.docx",
        language: "fr",
        theme: "light",
      });

      expect(screen.getByText("100%")).toBeInTheDocument();
      const zoomInBtn = screen.getByTitle("Zoom avant");
      await fireEvent.click(zoomInBtn);
      expect(screen.getByText("110%")).toBeInTheDocument();
    });
  });

  describe("CodeMarkdownViewer", () => {
    it("renders code with line numbers and markdown view toggle", async () => {
      render(CodeMarkdownViewer, {
        content: "# Titre Markdown\n\nTexte d'introduction.",
        fileName: "notes.md",
        mimeType: "text/markdown",
        language: "fr",
        theme: "dark",
      });

      expect(screen.getByText("notes.md")).toBeInTheDocument();
      const sourceBtn = screen.getByText("Source");
      await fireEvent.click(sourceBtn);
      expect(screen.getByText(/# Titre Markdown/)).toBeInTheDocument();
    });
  });

  describe("MediaViewer", () => {
    it("renders audio player for audio files", () => {
      render(MediaViewer, {
        url: "blob:audio",
        fileName: "recording.mp3",
        mimeType: "audio/mp3",
        theme: "light",
      });

      expect(screen.getByText("recording.mp3")).toBeInTheDocument();
    });

    it("renders video element for video files", () => {
      const { container } = render(MediaViewer, {
        url: "blob:video",
        fileName: "demo.mp4",
        mimeType: "video/mp4",
        theme: "light",
      });

      expect(container.querySelector("video.video-element")).toHaveAttribute("src", "blob:video");
    });
  });

  describe("AgentStepCard", () => {
    it("renders inline browser card with URL and navigate trigger", async () => {
      const dispatchSpy = vi.spyOn(window, "dispatchEvent");
      const step: AgentStep = {
        id: "step-1",
        runId: "run-1",
        sequence: 1,
        kind: "tool",
        status: "completed",
        title: "Navigation vers Wikipédia",
        input: {
          toolId: "core.browser.navigate",
          url: "https://en.wikipedia.org/wiki/Artificial_intelligence",
        },
        output: {
          title: "Artificial intelligence - Wikipedia",
          status: 200,
          content: "L'intelligence artificielle est un ensemble de technologies...",
        },
        startedAt: new Date().toISOString(),
      };

      render(AgentStepCard, {
        step,
        messageId: "msg-1",
        isExpanded: true,
        language: "fr",
        theme: "light",
      });

      expect(screen.getByText("Navigation vers Wikipédia")).toBeInTheDocument();
      expect(screen.getByText("https://en.wikipedia.org/wiki/Artificial_intelligence")).toBeInTheDocument();
      expect(screen.getByText("Artificial intelligence - Wikipedia")).toBeInTheDocument();

      const browseBtn = screen.getByText("Naviguer");
      await fireEvent.click(browseBtn);
      expect(dispatchSpy).toHaveBeenCalledWith(
        expect.objectContaining({
          type: "aro:open-browser",
          detail: { url: "https://en.wikipedia.org/wiki/Artificial_intelligence" },
        })
      );
    });

    it("renders document output card with visualize button", async () => {
      const dispatchSpy = vi.spyOn(window, "dispatchEvent");
      const step: AgentStep = {
        id: "step-2",
        runId: "run-1",
        sequence: 2,
        kind: "tool",
        status: "completed",
        title: "Génération du tableur Excel",
        input: { toolId: "core.document.create", format: "excel" },
        output: {
          format: "excel",
          title: "Rapport Financier.xlsx",
          sizeBytes: 15420,
          artifact: {
            title: "Rapport Financier.xlsx",
            kind: "excel",
            mimeType: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
          },
        },
        startedAt: new Date().toISOString(),
      };

      render(AgentStepCard, {
        step,
        messageId: "msg-1",
        isExpanded: true,
        language: "fr",
        theme: "light",
      });

      expect(screen.getByText("Rapport Financier.xlsx")).toBeInTheDocument();
      expect(screen.getByText("EXCEL")).toBeInTheDocument();

      const viewBtn = screen.getByText("Visualiser");
      await fireEvent.click(viewBtn);
      expect(dispatchSpy).toHaveBeenCalledWith(
        expect.objectContaining({
          type: "aro:preview-file",
        })
      );
    });
  });

  describe("PreviewOverlays", () => {
    it("renders spreadsheet preview for CSV file", async () => {
      const file: PreviewFile = {
        name: "data.csv",
        mimeType: "text/csv",
        url: "blob:csv",
        content: "A,B,C\n1,2,3",
      };

      render(PreviewOverlays, {
        file,
        codeOpen: false,
        codeHtml: "",
        loading: false,
        language: "fr",
        theme: "light",
        onCloseFile: () => {},
        onCloseCode: () => {},
      });

      expect(screen.getAllByText("data.csv").length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText("A").length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText("1").length).toBeGreaterThanOrEqual(1);
    });

    it("renders clear error box when file parsing fails", async () => {
      const biff8Data = new Uint8Array([0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]);
      const file: PreviewFile = {
        name: "legacy.xls",
        mimeType: "application/vnd.ms-excel",
        url: "blob:xls",
        data: biff8Data,
      };

      render(PreviewOverlays, {
        file,
        codeOpen: false,
        codeHtml: "",
        loading: false,
        language: "fr",
        theme: "light",
        onCloseFile: () => {},
        onCloseCode: () => {},
      });

      // Wait for promise resolution
      await new Promise((r) => setTimeout(r, 50));

      expect(screen.getByText("Impossible d'afficher l'aperçu")).toBeInTheDocument();
      expect(screen.getByText(/BIFF8/)).toBeInTheDocument();
      expect(screen.getByText("Télécharger le fichier")).toBeInTheDocument();
    });
  });
});
