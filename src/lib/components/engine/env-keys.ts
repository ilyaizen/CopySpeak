// Pure decision logic for the env-key override warning. When a cloud engine's
// key currently comes from .env (get_env_key_sources) and the user typed a
// different key, saving is allowed but must be confirmed: env precedence
// (secrets::resolve is env-first) keeps the .env key active until it is
// removed there and the app restarts.
import type { CredentialTarget } from "./engine-meta";

export interface EnvKeyConflict {
  engine: CredentialTarget;
  envVar: string;
}

/**
 * Decide whether saving `current` for an env-sourced engine needs the
 * override warning. Callers only invoke this for engines present in
 * get_env_key_sources; returns null when there is nothing to warn about:
 * no key typed, or the value unchanged from what is already persisted.
 */
export function firstEnvKeyConflict(
  engine: string,
  envVar: string,
  current: { api_key?: string } | undefined,
  original: { api_key?: string } | undefined
): EnvKeyConflict | null {
  const typed = (current?.api_key ?? "").trim();
  if (!typed) return null;
  const saved = (original?.api_key ?? "").trim();
  if (typed === saved) return null;
  // SAFETY: the engine ids come from get_env_key_sources, whose key set is
  // the same cloud-engine list the CredentialTarget union mirrors.
  return { engine: engine as CredentialTarget, envVar };
}
