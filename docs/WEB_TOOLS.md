# Web Tools Architecture

ARO now has a shared tool execution layer for web-aware agents.

## Capabilities

- `web.search`: searches public web pages and returns ranked source snippets.
- `web.fetch`: fetches a public HTTP(S) page, extracts readable text, stores a page snapshot artifact, and exposes a compact context source.
- Tool results are persisted as agent steps, context items, and artifacts so later runs can inspect what happened.
- Desktop chat can run a multi-turn tool loop: model JSON action, tool execution, context rebuild, final answer.
- API runs and `/v1/assistant/stream` can prefetch web context and expose `/v1/agent/tools/execute` for future server-side model workers.

## Safety Model

`aro-tools` blocks common SSRF paths before fetching:

- only `http` and `https` URLs are accepted;
- URLs with credentials are rejected;
- DNS is resolved before fetch;
- the validated DNS addresses are pinned into the request client and redirects are revalidated;
- loopback, private, link-local, multicast, unspecified, and metadata-service IPs are rejected;
- ambient `HTTP_PROXY`/`HTTPS_PROXY` variables are ignored so a proxy cannot bypass local DNS validation;
- content type and body size are bounded;
- extracted page text is truncated to configured limits.

Domain allow-lists support exact domains, wildcard `*`, and subdomain patterns like `*.example.com`.
Requested domains only narrow the persisted permission profile; they can never broaden it. `webAccess: "auto"` is deny-by-default and performs no prefetch or network call. Only the explicit `"on"` mode can enable web access.

## Runtime Flow

1. A message creates an agent run and records a `run-started` step.
2. URLs in the user request may be prefetched with `web.fetch`.
3. Memory and web context are packed into the agent context.
4. The model receives a JSON action contract and the available tool schemas.
5. If the model calls `web.search` or `web.fetch`, ARO executes the tool, persists the result, rebuilds context, and asks the model again.
6. The final answer is stored as the assistant message and linked to the agent run.

The model/tool loop is unlimited: it keeps running model → tool → context rebuild until the model returns a final or pause action (or a validation/provider error). `max_steps` is accepted for backward compatibility but no longer bounds execution.

## Configuration

- `ARO_WEB_SEARCH_ENDPOINT`: optional SearxNG-compatible JSON endpoint. If unset, ARO uses DuckDuckGo HTML as a best-effort fallback.
- `ARO_WEB_USER_AGENT`: HTTP user agent for web tools.
- `ARO_WEB_TIMEOUT_MS`: request timeout, default `12000`.
- `ARO_WEB_MAX_FETCH_BYTES`: max response bytes, default `2097152`.
- `ARO_WEB_MAX_PAGE_CHARS`: max extracted page text, default `24000`.
- `ARO_WEB_SEARCH_LIMIT`: default search result count, default `6`.

System proxy discovery is intentionally disabled. Supporting an enterprise egress proxy requires a separately configured, trusted proxy path that preserves destination validation and auditability.

## API Surface

- `POST /v1/assistant/stream` accepts `webAccess: "off" | "auto" | "on"`.
- `POST /v1/agent/tools/execute` executes a tool for an existing run and persists the result.
- `POST /v1/agent/runs` creates a run; automatic web enrichment remains disabled unless access is explicitly enabled.

## Extension Points

The tool layer is intentionally provider-neutral. Future work can add:

- native OpenAI Responses API hosted tools;
- MCP connector tools;
- workspace file tools backed by permission profiles;
- cached search indexes for repeated domains;
- background scheduled browsing and monitors.
