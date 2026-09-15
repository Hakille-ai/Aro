export interface TokenStorage {
  getAccessToken(): Promise<string | null>;
  setAccessToken(token: string | null): Promise<void>;
  getRefreshToken(): Promise<string | null>;
  setRefreshToken(token: string | null): Promise<void>;
  clear(): Promise<void>;
}

export class MemoryTokenStorage implements TokenStorage {
  private accessToken: string | null = null;
  private refreshToken: string | null = null;

  async getAccessToken(): Promise<string | null> {
    return this.accessToken;
  }

  async setAccessToken(token: string | null): Promise<void> {
    this.accessToken = token;
  }

  async getRefreshToken(): Promise<string | null> {
    return this.refreshToken;
  }

  async setRefreshToken(token: string | null): Promise<void> {
    this.refreshToken = token;
  }

  async clear(): Promise<void> {
    this.accessToken = null;
    this.refreshToken = null;
  }
}
