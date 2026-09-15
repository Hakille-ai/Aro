/**
 * Backwards-compatible API facade.
 *
 * New code may import a domain module from `./api/*`; existing consumers can
 * keep importing this file without any signature or runtime behavior change.
 */
export * from "./api/bootstrap";
export * from "./api/auth-organizations";
export * from "./api/conversations-memory-files";
export * from "./api/agents-permissions-arena";
export * from "./api/settings-models-runtime";
export * from "./api/voice";
export * from "./api/integrations";
export * from "./api/plugins";
