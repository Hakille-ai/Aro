// chatActions.ts
// Robust global click and interaction handler for interactive forms, suggested reply chips, and tool buttons.

export function setupChatInteractions() {
  if (typeof window === "undefined") return;

  // Prevent duplicate installation
  if ((window as any).__aroChatInteractionsInstalled) return;
  (window as any).__aroChatInteractionsInstalled = true;

  let lastSubmittedPrompt = "";
  let lastSubmittedTime = 0;

  function submitPromptToChat(promptText: string) {
    if (!promptText || !promptText.trim()) return;
    const cleanText = promptText.trim();
    const now = Date.now();
    // Debounce duplicate submissions within 600ms
    if (cleanText === lastSubmittedPrompt && now - lastSubmittedTime < 600) {
      return;
    }
    lastSubmittedPrompt = cleanText;
    lastSubmittedTime = now;

    // 1. Direct function call if App.svelte exposed it
    const directFn = (window as any).__aroSendPrompt || (window as any).__sendChatOptionHandler;
    if (typeof directFn === "function") {
      try {
        directFn(cleanText);
        // Clear textarea if present
        const textarea = document.querySelector(".composer textarea") as HTMLTextAreaElement | null;
        if (textarea) {
          textarea.value = "";
          textarea.dispatchEvent(new Event("input", { bubbles: true }));
        }
        return;
      } catch (err) {
        console.error("Direct __aroSendPrompt error, falling back:", err);
      }
    }

    // 2. Dispatch custom event for App.svelte / listeners
    window.dispatchEvent(new CustomEvent("aro:send-prompt", { detail: { text: cleanText } }));
  }

  // Global capture click listener
  document.addEventListener(
    "click",
    (e: MouseEvent) => {
      const target = e.target as HTMLElement | null;
      if (!target) return;

      // A. Form option chips (e.g. "Bien, merci.")
      const chip = (target.closest(".form-option-chip") || target.closest("[data-option]")) as HTMLElement | null;
      if (chip) {
        e.preventDefault();
        e.stopPropagation();
        e.stopImmediatePropagation();

        let optionText = "";
        const raw = chip.getAttribute("data-option");
        if (raw) {
          try { optionText = decodeURIComponent(raw); } catch { optionText = raw; }
        }
        if (!optionText && chip.textContent) {
          optionText = chip.textContent.trim();
        }

        if (optionText) {
          // Visual click feedback
          chip.style.opacity = "0.5";
          chip.style.transform = "scale(0.95)";
          chip.style.pointerEvents = "none";
          setTimeout(() => {
            chip.style.opacity = "";
            chip.style.transform = "";
            chip.style.pointerEvents = "";
          }, 400);

          submitPromptToChat(optionText);
        }
        return;
      }

      // B. Form custom input submit button
      const submitBtn = (target.closest(".form-submit-btn") || target.closest("[data-form-submit]")) as HTMLElement | null;
      if (submitBtn) {
        e.preventDefault();
        e.stopPropagation();
        e.stopImmediatePropagation();
        const row = submitBtn.closest(".form-input-row");
        const inputEl = row?.querySelector(".form-text-input") as HTMLInputElement | null;
        if (inputEl && inputEl.value.trim()) {
          const val = inputEl.value.trim();
          inputEl.value = "";
          submitPromptToChat(val);
        }
        return;
      }

      // C. Tool execution buttons
      const toolBtn = (target.closest(".tool-card-act-btn") || target.closest("[data-tool]")) as HTMLElement | null;
      if (toolBtn) {
        e.preventDefault();
        e.stopPropagation();
        e.stopImmediatePropagation();
        const raw = toolBtn.getAttribute("data-tool");
        let toolName = "";
        if (raw) {
          try { toolName = decodeURIComponent(raw); } catch { toolName = raw; }
        }
        if (toolName) {
          submitPromptToChat(`Exécute l'outil ${toolName}`);
        }
        return;
      }
    },
    true // Capture phase: runs before any stopPropagation
  );

  // Global keydown listener for Enter key in form text inputs
  document.addEventListener(
    "keydown",
    (e: KeyboardEvent) => {
      if (e.key === "Enter") {
        const target = e.target as HTMLElement | null;
        if (target && target.classList.contains("form-text-input")) {
          e.preventDefault();
          e.stopPropagation();
          e.stopImmediatePropagation();
          const inputEl = target as HTMLInputElement;
          if (inputEl.value.trim()) {
            const val = inputEl.value.trim();
            inputEl.value = "";
            submitPromptToChat(val);
          }
        }
      }
    },
    true
  );

  // Global window functions
  (window as any).__sendChatOption = (target: string | HTMLElement) => {
    let optionText = "";
    if (typeof target === "string") {
      optionText = target;
    } else if (target) {
      const raw = target.getAttribute?.("data-option");
      if (raw) {
        try { optionText = decodeURIComponent(raw); } catch { optionText = raw; }
      }
      if (!optionText && target.textContent) {
        optionText = target.textContent.trim();
      }
    }
    if (optionText) {
      submitPromptToChat(optionText);
    }
  };

  (window as any).__sendChatInput = (inputId: string) => {
    const el = document.getElementById(inputId) as HTMLInputElement | null;
    if (el && el.value.trim()) {
      const val = el.value.trim();
      el.value = "";
      submitPromptToChat(val);
    }
  };

  (window as any).__useToolPrompt = (target: string | HTMLElement) => {
    let toolName = "";
    if (typeof target === "string") {
      toolName = target;
    } else if (target) {
      const raw = target.getAttribute?.("data-tool");
      if (raw) {
        try { toolName = decodeURIComponent(raw); } catch { toolName = raw; }
      }
    }
    if (toolName) {
      submitPromptToChat(`Exécute l'outil ${toolName}`);
    }
  };
}

// Auto-run immediately when module is evaluated
setupChatInteractions();
