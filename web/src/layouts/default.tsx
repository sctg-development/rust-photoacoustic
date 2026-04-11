import type React from "react";

import { Dropdown } from "@heroui/react";
import { Trans, useTranslation } from "react-i18next";
import { useEffect, useState } from "react";
import { createRemoteJWKSet, JWTPayload, jwtVerify } from "jose";

import { useAuth } from "../authentication";
import { Navbar } from "../components/navbar";
import { siteConfig } from "../config/site";
import { useGenerixConfig } from "../authentication/providers/generix-config";

export default function DefaultLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const { t } = useTranslation();
  const { isAuthenticated, user, getAccessToken } = useAuth();
  const [accessToken, setAccessToken] = useState<string | null>(null);
  const [decodedToken, setDecodedToken] = useState<JWTPayload | null>(null);

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
  }, [isAuthenticated, generixConfig]);

  return (
    <div className="relative flex flex-col h-screen">
      <Navbar />
      <main className="container mx-auto max-w-7xl px-6 flex-grow pt-16">
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
        <Dropdown>
          <Dropdown.Trigger>
            {isAuthenticated ? (
              <span className="cursor-pointer">
                {t("user")}: &nbsp;{user?.name}
              </span>
            ) : (
              <></>
            )}
          </Dropdown.Trigger>
          <Dropdown.Popover>
            <Dropdown.Menu className="max-w-5xl">
              <Dropdown.Item key="user-logged" textValue="user-logged">
                <span className="text-default-600">{t("token")}:</span>
                <br />
                <div className="max-w-4xl font-mono text-xs bg-default-100 rounded p-2 mt-1">
                  <div className="max-w-2xs sm:max-w-sm md:max-w-md lg:max-w-3xl whitespace-break-spaces text-wrap break-words">
                    {accessToken}
                  </div>
                </div>
                <br />
                <span className="text-default-600">
                  {t("expiration")}:{" "}
                  {new Date((decodedToken?.exp || 0) * 1000).toLocaleString()}
                </span>
                <br />
                <span className="text-default-600">
                  {t("permissions")}:{" "}
                  {((decodedToken?.permissions as string[]) || []).join(", ") ||
                    t("no-permissions")}
                </span>
              </Dropdown.Item>
            </Dropdown.Menu>
          </Dropdown.Popover>
        </Dropdown>
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
