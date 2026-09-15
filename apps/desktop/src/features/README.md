# Frontend feature structure

The desktop UI is organized by product feature. `App.svelte` is the composition
root: it coordinates shared application state and passes data and callbacks to
the feature components, but it no longer owns their markup or styling.

Each feature may contain:

- `*.svelte` for views and reusable UI components;
- `model.ts` for domain types, defaults, catalogs, and pure transformations;
- `model.test.ts` for model invariants;
- feature-specific controllers or services when a workflow grows beyond the
  composition root.

Cross-feature backend calls and transport details live in `src/lib/api/`.
Shared backend types live in `src/lib/types/`. The compatibility facades
`src/lib/api.ts` and `src/lib/types.ts` keep existing imports stable.

Global styles are split by their original cascade order in `src/styles/app/`.
`main.ts` must keep those imports ordered from `01-` through `07-`; changing
that order can change the visual result even when selectors remain identical.

When adding a page, create it under the relevant feature and leave only its
state wiring in `App.svelte`. Preserve the component tests and Playwright
goldens with `npm run test:frontend --workspace @aro/desktop`.
