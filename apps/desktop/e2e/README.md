# Frontend refactor guardrails

The browser checks run the Vite application with every `/v1/` request replaced by
a deterministic unauthenticated response. They do not require the Rust API or a
Tauri runtime.

From the repository root:

```powershell
npm run test:components
npx playwright install chromium
npm run test:e2e
```

`npm run test:frontend` runs the unit, component, browser-behaviour, and visual
contracts together. The visual suite covers desktop light, desktop dark, and
mobile dark authentication layouts.

`npm run test:visual:update` intentionally refreshes the committed golden image.
Use it only after confirming that a deliberate design change is expected; a
component extraction by itself must keep the existing golden image unchanged.
