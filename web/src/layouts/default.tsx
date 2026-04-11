import type React from "react";

import { Trans, useTranslation } from "react-i18next";
import { useEffect, useState } from "react";
import { createRemoteJWKSet, JWTPayload, jwtVerify } from "jose";

import { useAuth } from "../authentication";
import { Navbar } from "../components/navbar";
import { UserTechnicalInfoModal } from "../components/user-technical-info";
import { siteConfig } from "../config/site";
import { useGenerixConfig } from "../authentication/providers/generix-config";
import { TOKEN_REFRESHED_EVENT } from "../hooks/use-token-refresh";

export default function DefaultLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const { t } = useTranslation();
  const { isAuthenticated, user, getAccessToken } = useAuth();
  const [accessToken, setAccessToken] = useState<string | null>(null);
  const [decodedToken, setDecodedToken] = useState<JWTPayload | null>(null);
  const [isUserModalOpen, setIsUserModalOpen] = useState(false);

  const { config: generixConfig } = useGenerixConfig();

  useEffect(() => {
    if (isAuthenticated && generixConfig) {
      getAccessToken().then((token) => {
        setAccessToken(token);
        const jwksUrl = `${generixConfig.authority}/.well-known/jwks.json`;

        console.log("JWKS URL:", jwksUrl);
        const JWKS = createRemoteJWKSet(new URL(jwksUrl));

        if (token) {
          jwtVerify(token, JWKS, {
            issuer: `${generixConfig.issuer}`,
            audience: `${generixConfig.audience}`,
          }).then((jwt) => {
            setDecodedToken(jwt.payload as JWTPayload);
          });
        }
      });
    }
  }, [isAuthenticated, generixConfig, getAccessToken]);

  useEffect(() => {
    const handleTokenRefreshed = (event: CustomEvent) => {
      const newToken = event.detail;
      setAccessToken(newToken);
      const jwksUrl = `${generixConfig?.authority}/.well-known/jwks.json`;

      if (generixConfig && newToken) {
        const JWKS = createRemoteJWKSet(new URL(jwksUrl));
        jwtVerify(newToken, JWKS, {
          issuer: `${generixConfig.issuer}`,
          audience: `${generixConfig.audience}`,
        }).then((jwt) => {
          setDecodedToken(jwt.payload as JWTPayload);
        });
      }
    };

    window.addEventListener(
      TOKEN_REFRESHED_EVENT,
      handleTokenRefreshed as EventListener,
    );
    return () => {
      window.removeEventListener(
        TOKEN_REFRESHED_EVENT,
        handleTokenRefreshed as EventListener,
      );
    };
  }, [generixConfig]);

  return (
    <div className="relative flex flex-col h-screen">
      <Navbar />
      <main className="container mx-auto max-w-7xl px-6 grow pt-16">
        {children}
      </main>
      <footer className="w-full flex items-center justify-center py-3">
        <a
          className="flex items-center gap-1 text-current"
          href={siteConfig().links.brand}
          rel="noopener noreferrer"
          target="_blank"
          title={t("site-homepage")}
        >
          <span className="text-default-600">
            <Trans ns="base">powered-by</Trans>
          </span>
          <p className="text-primary">{t("brand")}&nbsp;</p>
        </a>
        {isAuthenticated && user ? (
          <>
            <span
              className="cursor-pointer text-foreground hover:text-primary transition-colors"
              onClick={() => setIsUserModalOpen(true)}
            >
              {t("user")}: &nbsp;{user.name}
            </span>
            <UserTechnicalInfoModal
              isOpen={isUserModalOpen}
              onClose={() => setIsUserModalOpen(false)}
              user={user}
              accessToken={accessToken}
              tokenPayload={decodedToken}
            />
          </>
        ) : null}
        <a
          className="flex items-center mx-1"
          color="secondary"
          href="/api/doc"
          target="_blank"
        >
          API
        </a>
      </footer>
    </div>
  );
}
