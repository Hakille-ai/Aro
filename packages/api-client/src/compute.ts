import type { ComputeCompletion, ComputeCompletionRequest } from '@aro/contracts';

/** Text-only ARO Compute client. Never uses account JWTs or invents fallback results. */
export class AroComputeClient {
  private readonly baseUrl: string;
  constructor(baseUrl: string, private readonly getKey: () => string | Promise<string>) {
    const url = new URL(baseUrl);
    const local = ['localhost','127.0.0.1','[::1]'].includes(url.hostname);
    if ((url.protocol !== 'https:' && !(url.protocol === 'http:' && local)) || url.username || url.password || url.search || url.hash) throw new Error('A secure ARO API base URL is required.');
    this.baseUrl = url.href.replace(/\/+$/, '').replace(/\/v1$/, '') + '/v1';
  }

  /** Persist requestId before sending. Retry an identical payload using the SAME id. */
  async complete(request: ComputeCompletionRequest, requestId: string, signal?: AbortSignal): Promise<ComputeCompletion> {
    if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(requestId)) throw new Error('requestId must be a UUID retained for retries.');
    const key = await this.getKey();
    if (!/^aro_compute_[0-9a-f]{64}$/.test(key)) throw new Error('An ARO Compute key is required.');
    const response = await fetch(`${this.baseUrl}/chat/completions`, {
      method: 'POST', redirect: 'error', signal,
      headers: {'Content-Type':'application/json', Authorization:`Bearer ${key}`, 'Idempotency-Key':requestId},
      body: JSON.stringify({...request, stream:false}),
    });
    if (!response.ok) throw new Error(`ARO Compute returned HTTP ${response.status}. Retain the request ID; inspect the account before sending a new request.`);
    return response.json() as Promise<ComputeCompletion>;
  }
}
