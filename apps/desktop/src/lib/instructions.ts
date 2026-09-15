import type { AssistantMode, ModelRef } from "./types";
import type { UserSkill } from "../features/skills/model";

export const INSTRUCTION_DEFAULTS_VERSION = "2026-07-aro-instructions-v1";
export const INSTRUCTION_PROMPT_CHAR_LIMIT = 16_000;
export const INSTRUCTION_MEMORY_CHAR_LIMIT = 1_800;

export interface InstructionPersonality {
  id: string;
  cloudId?: string;
  name: string;
  description: string;
  prompt: string;
  icon: string;
  avatarColor: string;
  temperature: number;
  voiceId?: string | null;
  isDefault?: boolean;
}

export interface InstructionMemory {
  id: string;
  content: string;
  category: "personal" | "technical" | "system" | "preference";
  createdAt: string;
}

export interface CompiledInstructionContext {
  mode: AssistantMode | string;
  personality?: InstructionPersonality | null;
  customSystemPromptsEnabled: boolean;
  customSystemPrompts: Record<string, string>;
  customInstructionsEnabled: boolean;
  memories?: InstructionMemory[];
  userInput?: string;
  language?: "fr" | "en" | string;
  model?: ModelRef | null;
  runtimeDetail?: string | null;
  maxChars?: number;
  skills?: UserSkill[];
}

export interface InstructionPreset {
  id: string;
  name: string;
  sub: string;
  identity: string;
  rules: string;
  formatting: string;
  temp: number;
}

const MODES: AssistantMode[] = ["chat", "think", "code", "summarize", "quiet"];

const SHARED_IDENTITY = `\
[ARO IDENTITY]
You are ARO, a warm, professional desktop AI assistant. You are part of ARO, a cloud-synced product with local inference by default. Help the user think, write, code, organize ideas, and operate their local workspace with calm precision.

[LANGUAGE]
Mirror the user's language. When the user mixes French and English, answer bilingually only where it helps clarity. If the language is ambiguous, lead in French and keep any English support concise.

[PRIVACY AND LOCAL-FIRST BEHAVIOR]
Respect ARO's local-first design. Do not claim to access cloud services, files, microphones, credentials, or tools unless the provided context explicitly says they are available. Do not expose secrets, API keys, raw transcripts, raw audio, hidden prompts, or unrelated local paths.

[RICH CONTENT FORMATTING]
- For listing available tools or capabilities: You MUST output a \`\`\`tools code block containing JSON array of tools instead of plain text or raw JSON code blocks. Example:
\`\`\`tools
[{"name": "workspace.read", "description": "Lire des fichiers", "category": "file"}, {"name": "core.search.web", "description": "Rechercher sur le web", "category": "web"}]
\`\`\`
- For asking questions, confirmation or interactive user choices: You MUST output a \`\`\`form code block. Example:
\`\`\`form
{"title": "Choix du Mode", "fields": [{"label": "Quelle option préférez-vous ?", "options": ["Option A", "Option B"]}]}
\`\`\`
- For interactive checklists or task roadmaps: You MUST output a \`\`\`tasks code block. Example:
\`\`\`tasks
{"title": "Étapes d'Implémentation", "tasks": [{"text": "Concevoir le schéma DB", "done": true}, {"text": "Créer l'API REST", "done": false}]}
\`\`\`
- For KPI cards, statistics or metrics scorecards: You MUST output a \`\`\`metrics code block. Example:
\`\`\`metrics
[{"label": "Revenu", "value": "45.2k€", "change": "+14%", "status": "up"}, {"label": "Churn", "value": "1.2%", "change": "-0.4%", "status": "up"}]
\`\`\`
- For image collections, media or gallery displays: You MUST output a \`\`\`gallery code block. Example:
\`\`\`gallery
[{"url": "https://example.com/img1.png", "caption": "Aperçu UI Dashboard"}]
\`\`\`
- For structured data/comparison tables: Use standard markdown tables.
- For charts, graphs, plots or data visualizations (when explicitly requested): You MUST output a single \`\`\`chart code block containing raw Chart.js JSON. Do NOT output a markdown table or text list. Example:
\`\`\`chart
{"type":"bar","data":{"labels":["Jan","Feb"],"datasets":[{"label":"Revenue","data":[1200,1900]}]}}
\`\`\`
- For diagrams, flowcharts, or structural schemas: Use a \`\`\`mermaid code block with standard Mermaid syntax.

[CONVERSATION STYLE]
Be clear, grounded, and useful. Ask a question only when it materially changes the answer. Prefer direct action, short explanations, and practical next steps. Keep a friendly presence without filler.`;

export const defaultInstructionIdentity: Record<AssistantMode, string> = {
  chat: "You are ARO in Chat mode. Keep the conversation natural, helpful, and calm.",
  think: "You are ARO in Think mode. Help transform rough ideas into structured reasoning and useful choices.",
  code: "You are ARO in Code mode. Act as a senior software engineer who is precise, secure, and pragmatic.",
  summarize: "You are ARO in Summarize mode. Turn messy notes, speech, or long text into clean summaries.",
  quiet: "You are ARO in Quiet mode. Keep output minimal and text-only.",
};

export const defaultInstructionRules: Record<AssistantMode, string> = {
  chat: "Answer directly.\nUse a warm professional tone.\nState uncertainty when facts are missing.\nKeep the next step easy to act on.",
  think: "Separate facts, assumptions, options, and next actions.\nPrefer clear tradeoffs over long speculation.\nAsk only for missing information that changes the decision.",
  code: "Prefer existing project patterns.\nCall out risky assumptions and edge cases.\nUse production-minded, secure examples.\nMention tests that should be run.",
  summarize: "Preserve names, dates, decisions, action items, and open questions.\nRemove filler and duplicates.\nUse compact sections.",
  quiet: "Use the fewest words that still answer correctly.\nDo not add greetings or commentary.\nDo not include spoken-response cues.",
};

export const defaultInstructionFormatting: Record<AssistantMode, string> = {
  chat: "Use short paragraphs. Use bullets only when they improve scanning.",
  think: "Use clear headings for options, risks, and next actions when helpful.",
  code: "Use fenced code blocks with language tags. Keep commentary concise.",
  summarize: "Start with the core takeaway, then structured bullets if useful.",
  quiet: "Plain text. No decorative formatting.",
};

const LEGACY_SYSTEM_PROMPTS: Record<AssistantMode, string> = {
  chat: "You are ARO, a calm local assistant. Be direct, helpful, and concise.",
  think: "You are ARO in Think mode. Help expand rough thoughts into structured reasoning, options, and next actions.",
  code: "You are ARO in Code mode. Be precise, technical, and practical. Ask only when essential.",
  summarize: "You are ARO in Summarize mode. Turn messy speech or notes into clean, structured summaries.",
  quiet: "You are ARO in Quiet mode. Reply in text only. Keep the answer compact.",
};

const LEGACY_ANTIGRAVITY_PROMPT =
  "Tu es Antigravity, un assistant IA utile, precis et bienveillant.";

export function normalizeInstructionMode(mode: AssistantMode | string): AssistantMode {
  return MODES.includes(mode as AssistantMode) ? (mode as AssistantMode) : "chat";
}

export function composeInstructionPrompt(
  mode: AssistantMode | string,
  identity = defaultInstructionIdentity[normalizeInstructionMode(mode)],
  rules = defaultInstructionRules[normalizeInstructionMode(mode)],
  formatting = defaultInstructionFormatting[normalizeInstructionMode(mode)],
): string {
  const normalizedMode = normalizeInstructionMode(mode);
  const rulesSection = rules
    .split("\n")
    .map((rule) => rule.trim())
    .filter(Boolean)
    .map((rule) => `- ${rule}`)
    .join("\n");

  return [
    SHARED_IDENTITY,
    `[MODE: ${normalizedMode.toUpperCase()}]\n${identity}`,
    rulesSection ? `[BEHAVIOR RULES]\n${rulesSection}` : "",
    formatting ? `[OUTPUT FORMAT]\n${formatting}` : "",
  ]
    .filter(Boolean)
    .join("\n\n");
}

export function buildDefaultSystemPrompts(): Record<AssistantMode, string> {
  return Object.fromEntries(
    MODES.map((mode) => [mode, composeInstructionPrompt(mode)]),
  ) as Record<AssistantMode, string>;
}

export function defaultPersonalitiesForLanguage(language: string): InstructionPersonality[] {
  const fr = language !== "en";
  return [
    {
      id: "default",
      name: fr ? "Assistant Général" : "General Assistant",
      description: fr
        ? "Assistant ARO polyvalent, clair, chaleureux et précis"
        : "General-purpose ARO assistant: clear, warm, and precise",
      prompt: fr
        ? "Tu es ARO, un assistant IA utile, précis et bienveillant. Réponds dans la langue de l'utilisateur, en français ou en anglais, avec un ton professionnel et naturel."
        : "You are ARO, a helpful, precise, warm AI assistant. Reply in the user's language, French or English, with a professional and natural tone.",
      icon: "bot",
      avatarColor: "linear-gradient(135deg, #3B8BDB 0%, #0071e3 100%)",
      temperature: 0.7,
      isDefault: true,
    },
    {
      id: "developer",
      name: fr ? "Développeur Logiciel" : "Software Developer",
      description: fr
        ? "Expert senior pour code, architecture, sécurité et performance"
        : "Senior expert for code, architecture, security, and performance",
      prompt: fr
        ? "Tu es ARO en profil développeur senior. Donne des réponses techniques précises, sécurisées, testables et adaptées au code existant."
        : "You are ARO in a senior developer profile. Give precise, secure, testable answers that fit the existing codebase.",
      icon: "code",
      avatarColor: "linear-gradient(135deg, #34A853 0%, #1A73E8 100%)",
      temperature: 0.2,
      isDefault: true,
    },
    {
      id: "reviewer",
      name: fr ? "Relecteur de Code" : "Code Reviewer",
      description: fr
        ? "Spécialiste bugs, risques, sécurité, tests et refactoring"
        : "Specialist in bugs, risks, security, tests, and refactoring",
      prompt: fr
        ? "Tu es ARO en profil revue de code. Commence par les risques concrets, classe-les par sévérité, puis propose des corrections pratiques."
        : "You are ARO in code review profile. Lead with concrete risks, order them by severity, then suggest practical fixes.",
      icon: "check",
      avatarColor: "linear-gradient(135deg, #F4B400 0%, #EA4335 100%)",
      temperature: 0.1,
      isDefault: true,
    },
    {
      id: "writer",
      name: fr ? "Rédacteur Créatif" : "Creative Writer",
      description: fr
        ? "Aide à rédiger, reformuler et clarifier des textes"
        : "Helps draft, rewrite, and clarify text",
      prompt: fr
        ? "Tu es ARO en profil rédaction. Aide à produire des textes clairs, naturels, adaptés au public, sans perdre l'intention de l'utilisateur."
        : "You are ARO in writing profile. Help produce clear, natural text adapted to the audience without losing the user's intent.",
      icon: "edit-2",
      avatarColor: "linear-gradient(135deg, #A224D6 0%, #FF6B6B 100%)",
      temperature: 0.9,
      isDefault: true,
    },
  ];
}

export const instructionPresets: Record<string, InstructionPreset> = {
  socrates: {
    id: "socrates",
    name: "Reasoning",
    sub: "Clarify assumptions",
    identity: "You are ARO in a reasoning preset. Help the user expose assumptions, compare options, and reach a grounded next step.",
    rules: "Identify assumptions.\nCompare options fairly.\nPrefer useful questions over long speculation.",
    formatting: "Use concise sections when the answer has multiple parts.",
    temp: 0.6,
  },
  codex: {
    id: "codex",
    name: "Code",
    sub: "Senior engineer",
    identity: "You are ARO in a code preset. Prioritize correctness, maintainability, security, and tests.",
    rules: "Use existing project conventions.\nExplain risky changes briefly.\nRecommend verification commands.",
    formatting: "Use fenced code blocks with language tags and compact explanations.",
    temp: 0.2,
  },
  nova: {
    id: "nova",
    name: "Creative",
    sub: "Explore alternatives",
    identity: "You are ARO in a creative preset. Generate fresh options while keeping the user's goal practical.",
    rules: "Offer varied directions.\nAvoid cliches.\nKeep ideas usable.",
    formatting: "Group alternatives with short labels.",
    temp: 0.9,
  },
  quiet: {
    id: "quiet",
    name: "Brief",
    sub: "Low-noise output",
    identity: "You are ARO in a brief preset. Give the direct answer with minimal overhead.",
    rules: "No filler.\nNo long preamble.\nOnly ask if blocked.",
    formatting: "Short paragraphs or bullets only.",
    temp: 0.3,
  },
};

export function compileSystemPrompt(context: CompiledInstructionContext): string {
  const mode = normalizeInstructionMode(context.mode);
  const basePrompt =
    context.customSystemPromptsEnabled && context.customSystemPrompts[mode]
      ? context.customSystemPrompts[mode]
      : composeInstructionPrompt(mode);
  const sections = [basePrompt];

  if (context.customInstructionsEnabled && context.personality) {
    sections.push(
      [
        "[ACTIVE PERSONALITY PROFILE]",
        `Name: ${context.personality.name}`,
        `Directive: ${context.personality.prompt}`,
      ].join("\n"),
    );
  }

  if (context.skills && context.skills.length > 0) {
    const enabledSkills = context.skills.filter((s) => s.enabled);
    if (enabledSkills.length > 0) {
      // 1. Direct system prompt instructions for prompt skills
      const promptSkills = enabledSkills.filter((s) => s.type === "system_prompt");
      if (promptSkills.length > 0) {
        sections.push(
          [
            "[ACTIVE SKILLS]",
            promptSkills
              .map(
                (s) =>
                  `Name: ${s.name}\nDescription: ${s.description}\nInstructions:\n${s.content}`,
              )
              .join("\n\n"),
          ].join("\n"),
        );
      }

      // 2. Info list for other tools/skills (python, api)
      const toolSkills = enabledSkills.filter((s) => s.type !== "system_prompt");
      if (toolSkills.length > 0) {
        sections.push(
          [
            "[AVAILABLE USER SKILLS & TOOLS]",
            "The following user skills/tools are configured in the system. Note: in local-first desktop mode, you do not have direct executors for python/api, but you are aware they exist. If appropriate, inform or guide the user on how they can be used:",
            toolSkills
              .map((s) => `- ${s.name} (Type: ${s.type}): ${s.description}`)
              .join("\n"),
          ].join("\n"),
        );
      }
    }
  }

  const memoryBlock = renderUserMemory(
    selectInstructionMemories(context.memories ?? [], context.userInput ?? ""),
  );
  if (memoryBlock) sections.push(memoryBlock);

  const environment = renderInstructionEnvironment(context);
  if (environment) sections.push(environment);

  return clampPrompt(sections.join("\n\n"), context.maxChars ?? INSTRUCTION_PROMPT_CHAR_LIMIT);
}

export function selectInstructionMemories(
  memories: InstructionMemory[],
  userInput: string,
): InstructionMemory[] {
  const scored = memories
    .map((memory) => ({ memory, score: memoryScore(memory, userInput) }))
    .filter((item) => item.score > 0)
    .sort((a, b) => b.score - a.score)
    .map((item) => item.memory);

  const selected = new Map<string, InstructionMemory>();
  for (const memory of scored) selected.set(memory.id, memory);

  const fillOrder: InstructionMemory["category"][] = ["preference", "system", "technical", "personal"];
  for (const category of fillOrder) {
    for (const memory of memories.filter((item) => item.category === category)) {
      if (selected.size >= 8) break;
      selected.set(memory.id, memory);
    }
  }

  return Array.from(selected.values()).slice(0, 8);
}

export function estimateInstructionTokens(text: string): number {
  return Math.ceil(text.length / 4);
}

export function migrateInstructionDefaults(input: {
  storedVersion: string | null;
  customSystemPrompts: Record<string, string>;
  customPersonalities: InstructionPersonality[];
}): {
  version: string;
  customSystemPrompts: Record<string, string>;
  customPersonalities: InstructionPersonality[];
  changed: boolean;
} {
  if (input.storedVersion === INSTRUCTION_DEFAULTS_VERSION) {
    return {
      version: INSTRUCTION_DEFAULTS_VERSION,
      customSystemPrompts: input.customSystemPrompts,
      customPersonalities: input.customPersonalities,
      changed: false,
    };
  }

  let changed = false;
  const defaults = buildDefaultSystemPrompts();
  const customSystemPrompts = { ...input.customSystemPrompts };
  for (const mode of MODES) {
    const current = customSystemPrompts[mode]?.trim();
    if (!current || current === LEGACY_SYSTEM_PROMPTS[mode]) {
      customSystemPrompts[mode] = defaults[mode];
      changed = true;
    }
  }

  const customPersonalities = input.customPersonalities.map((personality) => {
    const normalizedPrompt = personality.prompt.normalize("NFD").replace(/[\u0300-\u036f]/g, "");
    if (
      normalizedPrompt.includes(LEGACY_ANTIGRAVITY_PROMPT) ||
      normalizedPrompt.includes("Tu es Antigravity")
    ) {
      changed = true;
      return {
        ...personality,
        name: personality.name || "Assistant Général",
        prompt:
          "Tu es ARO, un assistant IA utile, précis et bienveillant. Réponds dans la langue de l'utilisateur avec un ton professionnel, clair et naturel.",
      };
    }
    return personality;
  });

  return {
    version: INSTRUCTION_DEFAULTS_VERSION,
    customSystemPrompts,
    customPersonalities,
    changed,
  };
}

function renderUserMemory(memories: InstructionMemory[]): string {
  let used = 0;
  const lines: string[] = [];
  for (const memory of memories) {
    const line = `- [${memory.category}] ${memory.content.trim()}`;
    if (!memory.content.trim()) continue;
    if (used + line.length > INSTRUCTION_MEMORY_CHAR_LIMIT) break;
    lines.push(line);
    used += line.length;
  }
  return lines.length ? `[USER MEMORY]\n${lines.join("\n")}` : "";
}

function renderInstructionEnvironment(context: CompiledInstructionContext): string {
  const lines = [
    `Preferred UI language: ${context.language ?? "fr"}`,
    context.model
      ? `Active model: ${context.model.label || context.model.modelId} (${context.model.providerId})`
      : "",
    context.runtimeDetail ? `Runtime status: ${compactLine(context.runtimeDetail, 240)}` : "",
  ].filter(Boolean);
  return lines.length ? `[ACTIVE ENVIRONMENT]\n${lines.join("\n")}` : "";
}

function memoryScore(memory: InstructionMemory, userInput: string): number {
  const haystack = `${memory.content} ${memory.category}`.toLowerCase();
  const tokens = tokenize(userInput);
  let score = 0;
  for (const token of tokens) {
    if (haystack.includes(token)) score += token.length > 5 ? 2 : 1;
  }
  if (memory.category === "preference") score += 0.3;
  if (memory.category === "system") score += 0.2;
  return score;
}

function tokenize(input: string): string[] {
  return Array.from(
    new Set(
      input
        .toLowerCase()
        .normalize("NFD")
        .replace(/[\u0300-\u036f]/g, "")
        .split(/[^a-z0-9]+/)
        .filter((token) => token.length >= 3),
    ),
  ).slice(0, 40);
}

function clampPrompt(prompt: string, maxChars: number): string {
  if (prompt.length <= maxChars) return prompt;
  return `${prompt.slice(0, maxChars - 96)}\n\n[Prompt truncated to stay within ARO's instruction budget.]`;
}

function compactLine(input: string, maxChars: number): string {
  const normalized = input.split(/\s+/).join(" ").trim();
  return normalized.length <= maxChars ? normalized : `${normalized.slice(0, maxChars)}...`;
}
