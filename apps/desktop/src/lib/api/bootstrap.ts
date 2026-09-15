/**
 * Shared API entry points that initialize or synchronize several domains.
 * The stateful transport itself remains private to `transport.ts`, so all
 * domain exports share one authentication session and one demo data store.
 */
export { bootstrap } from "./transport";
