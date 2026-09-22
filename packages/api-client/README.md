# ARO SDK — Apache-2.0

This package and `@aro/contracts` are licensed under Apache-2.0. A commercial integration of this SDK is permitted by that license; it does not grant a commercial license to ARO core or include hosted services.

## Managed compute

```typescript
import { AroComputeClient } from '@aro/api-client';

const compute = new AroComputeClient('https://your-aro-server/v1', () => getSecretFromYourVault());
const requestId = crypto.randomUUID();
// Persist requestId with this payload BEFORE sending. Reuse both unchanged on retry.
const result = await compute.complete({
  model: 'your-managed-model',
  messages: [{role: 'user', content: 'Bonjour'}],
  max_tokens: 512,
}, requestId);
console.log(result.choices[0].message.content);
```

Use an `aro_compute_…` key, never an upstream provider key or account JWT. The helper does not retry automatically. On timeout or HTTP 409, retain the request ID and inspect the reservation before creating another request. Budgets, membership and idempotency are enforced by the server. This helper returns completed text; native tools, images and audio are not supported by the current managed gateway. Never put a shared organization compute key in a publicly distributed frontend bundle.
