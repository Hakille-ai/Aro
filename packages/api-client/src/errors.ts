export class ApiError extends Error {
  public status: number;
  public code?: string;
  public details?: unknown;

  constructor(message: string, status = 500, code?: string, details?: unknown) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
    this.details = details;
    Object.setPrototypeOf(this, ApiError.prototype);
  }

  static fromResponse(status: number, data: unknown): ApiError {
    let message = `API request failed with status ${status}`;
    let code: string | undefined;

    if (data && typeof data === "object") {
      const obj = data as Record<string, unknown>;
      if (typeof obj.error === "string") {
        message = obj.error;
      } else if (typeof obj.message === "string") {
        message = obj.message;
      }
      if (typeof obj.code === "string") {
        code = obj.code;
      }
    }

    return new ApiError(message, status, code, data);
  }
}
