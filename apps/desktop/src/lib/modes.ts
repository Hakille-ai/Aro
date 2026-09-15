import Brain from "@lucide/svelte/icons/brain";
import Code2 from "@lucide/svelte/icons/code-2";
import ListChecks from "@lucide/svelte/icons/list-checks";
import MessageCircle from "@lucide/svelte/icons/message-circle";
import VolumeX from "@lucide/svelte/icons/volume-x";
import type { Component } from "svelte";
import type { AssistantMode } from "./types";

export interface ModeOption {
  id: AssistantMode;
  label: string;
  icon: Component;
}

export const modes: ModeOption[] = [
  { id: "chat", label: "Chat", icon: MessageCircle },
  { id: "think", label: "Think", icon: Brain },
  { id: "code", label: "Code", icon: Code2 },
  { id: "summarize", label: "Notes", icon: ListChecks },
  { id: "quiet", label: "Quiet", icon: VolumeX },
];
