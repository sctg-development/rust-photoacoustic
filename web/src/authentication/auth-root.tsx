/**
 * @copyright Copyright (c) 2024-2025 Ronan LE MEILLAT
 * @license AGPL-3.0-or-later
 */

import React from "react";
import { Auth0Provider } from "@auth0/auth0-react";

import { AuthProviderWrapper } from "./providers/use-auth";

// Provider types we support
export type AuthenticationType = "auth0" | "generix";

interface AuthenticationProviderProps {
  children: React.ReactNode;
  providerType: AuthenticationType;
  config?: any;
}

/**
 * Root authentication provider component that sets up the appropriate
 * authentication provider based on the specified type
 */
export const AuthenticationProvider: React.FC<AuthenticationProviderProps> = ({
  children,
  providerType = "auth0",
  config,
}) => {
  // Prefer config.provider if present
  const effectiveProviderType = config?.provider || providerType;

  // Set up Auth0 provider
  if (effectiveProviderType === "auth0") {
    // Auth0 uses the following environment variables:
    // AUTH0_DOMAIN
    // AUTH0_CLIENT_ID
    // AUTH0_AUDIENCE
    // AUTH0_SCOPE
    const redirectUri = new URL(
      import.meta.env.BASE_URL || "/",
      window.location.origin,
    ).toString();

    return (
      <Auth0Provider
        authorizationParams={{
          redirect_uri: redirectUri,
          audience: import.meta.env.AUTH0_AUDIENCE,
          scope: import.meta.env.AUTH0_SCOPE,
        }}
        clientId={import.meta.env.AUTH0_CLIENT_ID}
        domain={import.meta.env.AUTH0_DOMAIN}
      >
        <AuthProviderWrapper providerType={effectiveProviderType}>
          {children}
        </AuthProviderWrapper>
      </Auth0Provider>
    );
  }

  // For Dex and other providers, we'd need to set up their specific provider
  if (effectiveProviderType === "generix") {
    // Dex doesn't need an external provider wrapper like Auth0 does
    // Dex use the fowlowing environment variables:
    // GENERIX_AUTHORITY
    // GENERIX_CLIENT_ID
    // GENERIX_REDIRECT_URI
    // GENERIX_SCOPE
    // GENERIX_AUDIENCE
    // GENERIX_TOKEN_ISSUER
    // GENERIX_JWKS_ENDPOINT
    // GENERIX_DOMAIN
    return (
      <AuthProviderWrapper config={config} providerType={effectiveProviderType}>
        {children}
      </AuthProviderWrapper>
    );
  }

  // Default fallback - should not happen if proper validation is in place
  return (
    <div>
      <h1>Invalid authentication provider type: {providerType}</h1>
      <p>Please check your configuration.</p>
    </div>
  );
};
