import {afterEach, describe, expect, it, vi} from 'vitest';
import {AroComputeClient} from './compute';
import {AroApiClient} from './client';
const id='c944fa99-3749-4b7c-9189-b4253f5857a3';
const key='aro_compute_'+'a'.repeat(64);
afterEach(()=>vi.unstubAllGlobals());
describe('paid compute retry boundary',()=>{
  it('retains the caller request key, routes to v1 once and uses no account token',async()=>{
    const fetcher=vi.fn().mockResolvedValue({ok:true,json:async()=>({id:'result'})});vi.stubGlobal('fetch',fetcher);
    const client=new AroComputeClient('https://api.example.test/v1',()=>key);
    const request={model:'aro-text',messages:[{role:'user' as const,content:'hello'}]};
    await client.complete(request,id);await client.complete(request,id);
    for(const [url,options] of fetcher.mock.calls) {
      expect(url).toBe('https://api.example.test/v1/chat/completions');
      expect(options.headers['Idempotency-Key']).toBe(id);
      expect(options.headers.Authorization).toBe(`Bearer ${key}`);
      expect(options.redirect).toBe('error');
    }
  });
  it('does not silently retry, debit again or fabricate a response on a timeout',async()=>{
    const fetcher=vi.fn().mockRejectedValue(new Error('network'));vi.stubGlobal('fetch',fetcher);
    await expect(new AroComputeClient('http://localhost:8710',()=>key).complete({model:'m',messages:[]},id)).rejects.toThrow('network');
    expect(fetcher).toHaveBeenCalledTimes(1);
  });
  it('rejects unsafe URLs and wrong credential types before a request',async()=>{
    expect(()=>new AroComputeClient('http://external.example.test',()=>key)).toThrow();
    const fetcher=vi.fn();vi.stubGlobal('fetch',fetcher);
    await expect(new AroComputeClient('https://api.example.test',()=> 'account-jwt').complete({model:'m',messages:[]},id)).rejects.toThrow('Compute key');
    expect(fetcher).not.toHaveBeenCalled();
  });
  it('never supplies mock billing data even when SDK mock fallback is enabled',async()=>{
    vi.stubGlobal('fetch',vi.fn().mockRejectedValue(new Error('offline')));
    await expect(new AroApiClient({enableMockFallback:true}).request('/billing/account')).rejects.toThrow();
  });
});
