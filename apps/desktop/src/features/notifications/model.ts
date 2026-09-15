export interface ToastNotification {
  id: string;
  type: "agent-completed" | "info" | "success" | "error";
  title: string;
  body: string;
  conversationId?: string;
  createdAt: number;
}
