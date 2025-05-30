// Utility to load and cache generix.json config at runtime
// Usage: await getGenerixConfig()

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
