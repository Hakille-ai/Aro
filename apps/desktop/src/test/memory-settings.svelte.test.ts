import { describe, it, expect, vi } from "vitest";
import { render, fireEvent, screen } from "@testing-library/svelte";
import MemorySettings from "../features/settings/pages/MemorySettings.svelte";
import { makeDefaultMemorySettings } from "../lib/api/transport";

describe("MemorySettings component", () => {
  const mockLabels = {
    memoryTitle: "Mémoire",
    memoryDesc: "Système de mémoire cognitive de l'agent",
    memoryLimitLabel: "Limite",
    memorySearchPlaceholder: "Rechercher...",
    addMemoryBtn: "Ajouter un souvenir",
    catPersonal: "Personnel",
    catTechnical: "Technique",
    catSystem: "Système",
    catPreference: "Préférence",
    newMemoryPlaceholder: "Nouveau souvenir...",
    newMemoryCategoryLabel: "Catégorie",
    cancelBtn: "Annuler",
    saveBtn: "Enregistrer",
    noMemoriesFound: "Aucun souvenir trouvé",
  };

  it("renders MemorySettings without error", () => {
    const { container } = render(MemorySettings, {
      props: {
        t: mockLabels,
        currentLanguage: "fr",
        currentTheme: "dark",
        memoriesList: [
          {
            id: "mem-1",
            content: "L'utilisateur préfère le mode sombre",
            category: "preference",
            scope: "user",
            status: "approved",
            sourceMessageIds: [],
            salience: 0.9,
            pinned: true,
            createdAt: "2026-09-10T12:00:00Z",
            updatedAt: "2026-09-12T12:00:00Z",
            lastUsedAt: "2026-09-12T12:00:00Z",
            recallCount: 5,
          }
        ],
        episodesList: [
          {
            id: "ep-1",
            conversationId: "conv-1",
            turnStart: 1,
            turnEnd: 10,
            summary: "Discussion sur l'architecture AGI",
            keyDecisions: ["Adopter un modèle hybride"],
            entities: ["ARO Agent"],
            tokenCount: 120,
            createdAt: "2026-09-11T14:00:00Z",
            updatedAt: "2026-09-11T14:00:00Z",
          }
        ],
        memoryIndex: {
          mode: "auto",
          state: "active",
          qdrantUrl: "http://localhost:6333",
          embeddingProvider: "openai",
          embeddingModel: "text-embedding-3-small",
          indexedCount: 1,
        },
        memoryIndexBusy: false,
        memoryEntryLimit: 500,
        memorySearchQuery: "",
        selectedMemoryFilter: "all",
        showAddMemoryInline: false,
        newMemoryText: "",
        newMemoryCategory: "personal",
        newMemorySalience: 0.7,
        newMemoryPinned: false,
        editingMemoryId: null,
        editingMemoryText: "",
        refreshMemoryIndexStatus: vi.fn(),
        reindexMemories: vi.fn(),
        addMemory: vi.fn(),
        updateMemory: vi.fn(),
        deleteMemory: vi.fn(),
        togglePinMemory: vi.fn(),
        onRefreshEpisodes: vi.fn(),
        memorySettings: makeDefaultMemorySettings(),
        onSaveMemorySettings: vi.fn(),
      }
    });

    expect(container.querySelector(".cognitive-memory-container")).toBeTruthy();
    expect(screen.getByText("Mémoire Cognitive ARO")).toBeInTheDocument();
  });

  it("switches tabs between Configuration, Semantic, Episodic, and Architecture", async () => {
    render(MemorySettings, {
      props: {
        t: mockLabels,
        currentLanguage: "fr",
        currentTheme: "dark",
        memoriesList: [],
        episodesList: [],
        memoryIndex: null,
        memoryIndexBusy: false,
        memoryEntryLimit: 500,
        refreshMemoryIndexStatus: vi.fn(),
        reindexMemories: vi.fn(),
        addMemory: vi.fn(),
        updateMemory: vi.fn(),
        deleteMemory: vi.fn(),
        memorySettings: makeDefaultMemorySettings(),
      }
    });

    // Default tab is "Paramètres & Moteur"
    expect(screen.getByText(/⚡ Auto/)).toBeInTheDocument();

    // Click "Faits Sémantiques (Long Terme)"
    const semanticTab = screen.getByText("Faits Sémantiques (Long Terme)");
    await fireEvent.click(semanticTab);
    expect(screen.getByPlaceholderText("Rechercher...")).toBeInTheDocument();

    // Click "Épisodes & Sessions (Moyen Terme)"
    const episodicTab = screen.getByText("Épisodes & Sessions (Moyen Terme)");
    await fireEvent.click(episodicTab);
    expect(screen.getByText("Synthèses Épisodiques de Sessions")).toBeInTheDocument();

    // Click "Diagnostics & Architecture"
    const archTab = screen.getByText("Diagnostics & Architecture");
    await fireEvent.click(archTab);
    expect(screen.getByText("Architecture Cognitive & Budget de Contexte")).toBeInTheDocument();
  });
});
