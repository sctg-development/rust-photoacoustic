/**
 * @copyright Copyright (c) 2024-2025 Ronan LE MEILLAT
 * @license AGPL-3.0-or-later
 */

import { useTranslation } from "react-i18next";
import { UserManager, User, WebStorageStateStore, Log } from "oidc-client-ts";
import { useEffect, useState } from "react";
import { JWTPayload, jwtVerify, createRemoteJWKSet } from "jose";

import {
  AuthProvider,
  AuthUser,
  TokenOptions,
  LogoutOptions,
  LoginOptions,
  AuthProviderConfig,
} from "./auth-provider";

/**
 * Dex implementation of the AuthProvider interface using oidc-client-ts
 */
export const useGenerixProvider = (
  providedConfig?: AuthProviderConfig,
): AuthProvider => {
  // Active debug logging for OIDC client to help troubleshoot
  Log.setLogger(console);
  Log.setLevel(Log.DEBUG);

  const { t } = useTranslation("authentication");

  // Use providedConfig directly (from generix.json)
  const config: AuthProviderConfig = {
    authority: providedConfig?.authority || "",
    clientId: providedConfig?.clientId || "",
    redirectUri: providedConfig?.redirectUri || "",
    scope: providedConfig?.scope || "openid profile email",
    audience: providedConfig?.audience,
    tokenIssuer: providedConfig?.tokenIssuer,
    jwksEndpoint: providedConfig?.jwksEndpoint,
    domain: providedConfig?.domain || "",
  };

  const [user, setUser] = useState<User | null>(null);
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [userManager] = useState<UserManager>(() => {
    return new UserManager({
      authority: config.authority,
      client_id: config.clientId,
      redirect_uri: config.redirectUri,
      response_type: "code",
      scope: config.scope || "openid profile email",
      automaticSilentRenew: true,
      filterProtocolClaims: true, // Make sure we filter protocol claims like nonce
      loadUserInfo: true, // Load user info from userinfo endpoint
      userStore: new WebStorageStateStore({ store: window.localStorage }),
    });
  });

  useEffect(() => {
    const checkUser = async () => {
      try {
        setIsLoading(true);

        // Important: Check for code AND state parameters
        if (
          window.location.search.includes("code=") &&
          window.location.search.includes("state=")
        ) {
          console.log(t("detected-authorization-callback"));

          try {
            // Process the callback
            const user = await userManager.signinRedirectCallback();

            console.log(
              t("successfully-processed-signin-callback", {
                name: user.profile.name,
              }),
            );

            setUser(user);
            setIsAuthenticated(!!user?.access_token);

            // Redirect to the stored location or default
            const redirectPath =
              sessionStorage.getItem("redirect_after_login") || "/";

            sessionStorage.removeItem("redirect_after_login");
            window.history.replaceState({}, document.title, redirectPath);
          } catch (callbackError) {
            console.error(t("error-handling-redirect-callback"), callbackError);

            // Clean URL even on error
            window.history.replaceState(
              {},
              document.title,
              window.location.pathname,
            );

            setUser(null);
            setIsAuthenticated(false);
          }
        } else {
          // Regular page load - check if user is already logged in
          try {
            const currentUser = await userManager.getUser();

            if (currentUser && currentUser.access_token) {
              console.log(t("user-already-logged-in"), currentUser);

              setUser(currentUser);
              setIsAuthenticated(true);
            } else {
              console.log(t("no-authenticated-user-found"));

              // Check if we should auto login
              const shouldAutoLogin =
                import.meta.env.GENERIX_AUTO_LOGIN !== "false";

              if (
                shouldAutoLogin &&
                !window.location.pathname.includes("/callback")
              ) {
                console.log(t("initiating-automatic-login-flow"));

                // Store the current location to return after login
                sessionStorage.setItem(
                  "redirect_after_login",
                  window.location.pathname + window.location.search,
                );

                // Redirect to Dex login - important to await this
                try {
                  await userManager.signinRedirect();
                  console.log(t("signin-redirect-initiated"));

                  return; // Return early as we're redirecting
                } catch (redirectError) {
                  console.error(
                    t("error-initiating-signin-redirect"),
                    redirectError,
                  );
                }
              }

              setUser(null);
              setIsAuthenticated(false);
            }
          } catch (error) {
            console.error(t("error-checking-user-session"), error);
            setUser(null);
            setIsAuthenticated(false);
          }
        }
      } catch (error) {
        console.error(t("authentication-error"), error);
      } finally {
        setIsLoading(false);
      }
    };

    checkUser();

    // Set up event listeners for user session changes
    const addUserSignedIn = (user: User) => {
      console.log(t("user-signed-in-event-received"), user);
      setUser(user);
      setIsAuthenticated(true);
    };

    const addUserSignedOut = () => {
      console.log(t("user-signed-out-event-received"));
      setUser(null);
      setIsAuthenticated(false);
    };

    userManager.events.addUserLoaded(addUserSignedIn);
    userManager.events.addUserUnloaded(addUserSignedOut);
    // Also listen for token expiration
    userManager.events.addAccessTokenExpiring(() => {
      console.log(t("access-token-expiring-soon"));
    });
    userManager.events.addAccessTokenExpired(() => {
      console.log(t("access-token-expired"));
    });

    return () => {
      userManager.events.removeUserLoaded(addUserSignedIn);
      userManager.events.removeUserUnloaded(addUserSignedOut);
      userManager.events.removeAccessTokenExpiring(() => { });
      userManager.events.removeAccessTokenExpired(() => { });
    };
  }, [userManager]);

  const login = async (options?: LoginOptions): Promise<void> => {
    try {
      console.log(t("login-initiated"), options);

      // Store the current path to redirect back after login
      if (window.location.pathname !== "/callback") {
        sessionStorage.setItem(
          "redirect_after_login",
          window.location.pathname + window.location.search,
        );
      }

      await userManager.signinRedirect(options);
      console.log(t("redirect-to-authentication-provider-initiated"));
    } catch (error) {
      console.error(t("error-during-login"), error);
      throw error;
    }
  };

  const logout = async (options?: LogoutOptions): Promise<void> => {
    try {
      console.log(t("logout-initiated"), options);

      // Clear any stored redirect paths
      sessionStorage.removeItem("redirect_after_login");

      await userManager.signoutRedirect({
        post_logout_redirect_uri:
          options?.logoutParams?.returnTo ||
          new URL(
            import.meta.env.BASE_URL || "/",
            window.location.origin,
          ).toString(),
      });

      console.log(t("redirect-to-logout-initiated"));
    } catch (error) {
      console.error(t("error-during-logout"), error);
      throw error;
    }
  };

  const getAccessToken = async (
    _options?: TokenOptions,
  ): Promise<string | null> => {
    try {
      const currentUser = await userManager.getUser();

      if (!currentUser || !currentUser.access_token) {
        console.log(t("no-access-token-available"));

        // Token is missing - we should authenticate
        // But this might cause infinite loops if called repeatedly
        // So only redirect if it's an explicit token request (not background check)
        if (_options?.redirect !== false) {
          console.log(t("initiating-login-to-obtain-access-token"));
          login();
        }

        return null;
      }

      // Check if token is expired
      if (
        currentUser.expires_at &&
        currentUser.expires_at < Date.now() / 1000
      ) {
        console.log(t("access-token-expired-attempting-silent-refresh"));

        try {
          // Try to silently refresh the token
          const newUser = await userManager.signinSilent();

          console.log(t("silent-token-refresh-successful"));

          return newUser?.access_token || null;
        } catch (silentError) {
          console.error(t("silent-token-refresh-failed"), silentError);

          // Silent refresh failed, redirect to login
          if (_options?.redirect !== false) {
            console.warn(
              t("token-expired-silent-refresh-failed-redirecting-to-login"),
            );
            login();
          }

          return null;
        }
      }

      return currentUser.access_token;
    } catch (error) {
      console.error(t("error-getting-access-token"), error);

      return null;
    }
  };

  const hasPermission = async (permission: string): Promise<boolean> => {
    try {
      const accessToken = await getAccessToken({ redirect: false });

      if (!accessToken) {
        return false;
      }

      const jwksEndpoint =
        config.jwksEndpoint || `${config.authority}/.well-known/jwks.json`;

      const JWKS = createRemoteJWKSet(new URL(jwksEndpoint));

      const joseResult = await jwtVerify(accessToken, JWKS, {
        issuer: config.tokenIssuer || config.authority,
        audience: config.audience,
      });

      const payload = joseResult.payload as JWTPayload;

      // Check permissions based on token format (permissions array, scope string, or custom fields)
      if (payload.permissions instanceof Array) {
        return payload.permissions.includes(permission);
      } else if (typeof payload.scope === "string") {
        return payload.scope.split(" ").includes(permission);
      } else if (
        payload.realm_access &&
        Array.isArray((payload.realm_access as any).roles)
      ) {
        return (payload.realm_access as any).roles.includes(permission);
      }

      return false;
    } catch (error) {
      console.error(t("error-checking-permission"), error);

      return false;
    }
  };

  // Helper methods for working with secured APIs
  const getJson = async (url: string): Promise<any> => {
    try {
      const accessToken = await getAccessToken();

      if (!accessToken) {
        throw new Error(t("not-authenticated"));
      }

      const response = await fetch(url, {
        headers: {
          Authorization: `Bearer ${accessToken}`,
        },
      });

      if (!response.ok) {
        throw new Error(
          t("http-error-status-text", {
            status: response.status,
            statusText: response.statusText,
          }),
        );
      }

      return await response.json();
    } catch (error) {
      console.error(t("error-fetching-json"), error);
      throw error;
    }
  };

  const postJson = async (url: string, data: any): Promise<any> => {
    try {
      const accessToken = await getAccessToken();

      if (!accessToken) {
        throw new Error(t("not-authenticated"));
      }

      const response = await fetch(url, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${accessToken}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify(data),
      });

      if (!response.ok) {
        throw new Error(
          t("http-error-status-text", {
            status: response.status,
            statusText: response.statusText,
          }),
        );
      }

      return await response.json();
    } catch (error) {
      console.error(t("error-posting-json"), error);
      throw error;
    }
  };

  const deleteJson = async (url: string): Promise<any> => {
    try {
      const accessToken = await getAccessToken();

      if (!accessToken) {
        throw new Error(t("not-authenticated"));
      }

      const response = await fetch(url, {
        method: "DELETE",
        headers: {
          Authorization: `Bearer ${accessToken}`,
          "Content-Type": "application/json",
        },
      });

      if (!response.ok) {
        throw new Error(
          t("http-error-status-text", {
            status: response.status,
            statusText: response.statusText,
          }),
        );
      }

      return await response.json();
    } catch (error) {
      console.error(t("error-deleting-json"), error);
      throw error;
    }
  };

  // Map OIDC user to common AuthUser format
  const authUser: AuthUser | null = user
    ? {
      name: user.profile.name,
      nickname: user.profile.nickname || user.profile.preferred_username,
      email: user.profile.email,
      ...user.profile,
    }
    : null;

  return {
    isAuthenticated,
    isLoading,
    user: authUser,
    login,
    logout,
    getAccessToken,
    hasPermission,
    getJson,
    postJson,
    deleteJson,
  };
};
