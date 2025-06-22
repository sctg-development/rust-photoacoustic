// Utility to load and cache generix.json config at runtime
// Usage: await getGenerixConfig()

import { useState, useEffect, useCallback } from "react";

export type RustGenerixConfig = {
  provider: string;
  api_base_url: string;
  authority: string;
  client_id: string;
  scope: string;
  redirect_uri: string;
  audience: string;
  token_issuer: string;
  jwks_endpoint: string;
  domain: string;
  issuer: string;
};

export type GenerixConfig = {
  provider: string;
  api_base_url: string;
  authority: string;
  clientId: string;
  scope: string;
  redirectUri: string;
  audience: string;
  token_issuer: string;
  jwks_endpoint: string;
  domain: string;
  issuer: string;
};

let generixConfigCache: GenerixConfig | null = null;

export async function getGenerixConfig(): Promise<GenerixConfig | null> {
  if (generixConfigCache) return generixConfigCache;
  const resp = await fetch(`${import.meta.env.BASE_URL}/generix.json`);
  const parsedConfig: RustGenerixConfig = await resp.json();

  if (!parsedConfig) return null;
  // Convert RustGenerixConfig to GenerixConfig
  generixConfigCache = {
    provider: parsedConfig.provider,
    api_base_url: parsedConfig.api_base_url,
    authority: parsedConfig.authority,
    clientId: parsedConfig.client_id,
    scope: parsedConfig.scope,
    redirectUri: parsedConfig.redirect_uri,
    audience: parsedConfig.audience,
    token_issuer: parsedConfig.token_issuer,
    jwks_endpoint: parsedConfig.jwks_endpoint,
    domain: parsedConfig.domain,
    issuer: parsedConfig.issuer,
  };

  return generixConfigCache;
}

// Hook pour gérer le chargement de la configuration Generix
export interface UseGenerixConfigOptions {
  /** Chargement automatique au montage du composant */
  autoLoad?: boolean;
  /** Callback en cas d'erreur */
  onError?: (error: Error) => void;
  /** Callback en cas de succès */
  onSuccess?: (config: GenerixConfig) => void;
}

export interface UseGenerixConfigReturn {
  /** Configuration Generix actuelle */
  config: GenerixConfig | null;
  /** État de chargement */
  loading: boolean;
  /** Erreur de chargement */
  error: string | null;
  /** Fonction pour recharger la configuration */
  reload: () => Promise<void>;
  /** Fonction pour charger la configuration manuellement */
  load: () => Promise<void>;
}

/**
 * Hook personnalisé pour gérer le chargement de la configuration Generix
 * 
 * @example
 * ```tsx
 * // Chargement automatique
 * const { config, loading, error } = useGenerixConfig();
 * 
 * // Chargement manuel
 * const { config, loading, error, load } = useGenerixConfig({ autoLoad: false });
 * useEffect(() => {
 *   if (someCondition) {
 *     load();
 *   }
 * }, [someCondition]);
 * 
 * // Avec callbacks
 * const { config } = useGenerixConfig({
 *   onError: (error) => console.error('Config error:', error),
 *   onSuccess: (config) => console.log('Config loaded:', config)
 * });
 * ```
 */
export function useGenerixConfig(options: UseGenerixConfigOptions = {}): UseGenerixConfigReturn {
  const { autoLoad = true, onError, onSuccess } = options;

  const [config, setConfig] = useState<GenerixConfig | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);

      const loadedConfig = await getGenerixConfig();

      setConfig(loadedConfig);

      if (loadedConfig && onSuccess) {
        onSuccess(loadedConfig);
      }
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : "Failed to load Generix configuration";
      setError(errorMessage);

      if (onError) {
        onError(err instanceof Error ? err : new Error(errorMessage));
      }
    } finally {
      setLoading(false);
    }
  }, [onError, onSuccess]);

  const reload = useCallback(async () => {
    // Vider le cache pour forcer le rechargement
    generixConfigCache = null;
    await load();
  }, [load]);

  // Chargement automatique au montage
  useEffect(() => {
    if (autoLoad) {
      load();
    }
  }, [autoLoad, load]);

  return {
    config,
    loading,
    error,
    reload,
    load,
  };
}
