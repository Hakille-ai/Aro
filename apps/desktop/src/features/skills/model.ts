export interface UserSkillGroup {
  id: string;
  cloudId?: string;
  name: string;
  description: string;
  createdAt: string;
}
export interface UserSkill {
  id: string;
  cloudId?: string;
  name: string;
  description: string;
  icon: string;
  category: string;
  groupId: string;
  triggers: string;
  type: "system_prompt" | "python" | "api";
  content: string;
  enabled: boolean;
  createdAt: string;
  pluginId?: string;
  pluginName?: string;
  tags?: string[];
  isPlugin?: boolean;
}
export function createInitialSkillGroups(): UserSkillGroup[] {
  return [
    {
      id: "all",
      name: "Tous les Skills",
      description: "Toutes vos compétences configurées",
      createdAt: "2026-06-20T08:00:00.000Z"
    },
    {
      id: "grp-1",
      name: "Utilitaires",
      description: "Outils de calcul et d'aide quotidienne",
      createdAt: "2026-06-20T08:00:00.000Z"
    },
    {
      id: "grp-2",
      name: "Langues",
      description: "Compétences de traduction linguistique",
      createdAt: "2026-06-25T12:00:00.000Z"
    }
    ];
}

export function createInitialSkills(): UserSkill[] {
  return [
    {
      id: "1",
      name: "Assistant de Calcul",
      description: "Guide le modèle pas à pas pour structurer les calculs arithmétiques complexes.",
      icon: "🧮",
      category: "Productivité",
      groupId: "grp-1",
      triggers: "",
      type: "system_prompt",
      content: "You are a calculation helper. Explain mathematical solutions step-by-step with clear formulas.",
      enabled: true,
      createdAt: "2026-06-20T08:00:00.000Z"
    },
    {
      id: "2",
      name: "Correcteur d'Orthographe",
      description: "Corrige l'orthographe et la grammaire sans modifier le style de l'auteur.",
      icon: "✍️",
      category: "Rédaction",
      groupId: "all",
      triggers: "",
      type: "system_prompt",
      content: "You are a professional editor. Correct any typos, grammar mistakes, and syntax issues. Preserve the tone, vocabulary, and length.",
      enabled: true,
      createdAt: "2026-06-25T12:30:00.000Z"
    },
    {
      id: "3",
      name: "Traducteur Académique",
      description: "Force l'assistant à adopter un ton universitaire rigoureux et neutre pour toutes ses traductions.",
      icon: "🌐",
      category: "Langues",
      groupId: "grp-2",
      triggers: "",
      type: "system_prompt",
      content: "You are an academic translator. Translate the text directly and accurately. Use formal vocabulary, avoid abbreviations, and maintain absolute neutrality.",
      enabled: false,
      createdAt: "2026-06-28T10:15:00.000Z"
    }
    ];
}
export function skillGroupPayload(group: UserSkillGroup) {
  return { clientId: group.id, name: group.name, description: group.description };
}
export function skillPayload(skill: UserSkill) {
  return {
    clientId: skill.id, clientGroupId: skill.groupId, name: skill.name, description: skill.description,
    icon: skill.icon, category: skill.category,
    triggers: skill.triggers.split(",").map((item) => item.trim()).filter(Boolean),
    kind: skill.type, content: skill.content, enabled: skill.enabled,
  };
}
