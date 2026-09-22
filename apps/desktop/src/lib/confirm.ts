import { writable } from "svelte/store";

export interface ConfirmOptions {
  title: string;
  body?: string;
  message?: string;
  confirmLabel?: string;
  confirmText?: string;
  cancelLabel?: string;
  cancelText?: string;
  /** Style destructif (rouge) pour le bouton de confirmation. */
  danger?: boolean;
}

interface PendingConfirm extends Required<Omit<ConfirmOptions, "body" | "message" | "confirmText" | "cancelText">> {
  body: string;
  resolve: (value: boolean) => void;
}

/** Requête de confirmation en cours (null = aucune). */
export const confirmRequest = writable<PendingConfirm | null>(null);

/**
 * Ouvre la modale de confirmation globale (montée une fois dans App) et
 * résout `true` si l'utilisateur confirme. Remplace `confirm()` natif qui
 * bloque l'UI Tauri et est inutilisable en tests.
 */
export function requestConfirm(options: ConfirmOptions): Promise<boolean> {
  return new Promise((resolve) => {
    confirmRequest.update((previous) => {
      // Un double-clic ne doit jamais laisser une promesse orpheline :
      // la requête précédente est refusée proprement.
      previous?.resolve(false);
      return {
        title: options.title,
        body: options.body ?? options.message ?? "",
        confirmLabel: options.confirmLabel ?? options.confirmText ?? "Confirmer",
        cancelLabel: options.cancelLabel ?? options.cancelText ?? "Annuler",
        danger: options.danger ?? true,
        resolve,
      };
    });
  });
}

export function resolveConfirm(value: boolean): void {
  confirmRequest.update((pending) => {
    pending?.resolve(value);
    return null;
  });
}
