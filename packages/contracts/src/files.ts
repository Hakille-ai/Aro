export interface FileObject {
  id: string;
  name: string;
  size: number;
  mimeType: string;
  storagePath?: string;
  conversationId?: string | null;
  uploadedAt: string;
}

export interface WorkspaceTreeEntry {
  name: string;
  path: string;
  relativePath: string;
  isDir: boolean;
  size?: number;
  extension: string;
  children?: WorkspaceTreeEntry[];
}

export interface FileQuotaView {
  usedBytes: number;
  maxBytes: number;
  usedPercentage: number;
  fileCount: number;
}
