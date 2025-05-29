import type React from "react";

import { Link } from "@heroui/link";
import { Trans, useTranslation } from "react-i18next";
import {
  Dropdown,
  DropdownItem,
  DropdownMenu,
  DropdownTrigger,
} from "@heroui/dropdown";
import { useAuth } from "@/authentication";
import { useEffect, useState } from "react";
import { Snippet } from "@heroui/snippet";
import { createRemoteJWKSet, JWTPayload, jwtVerify } from "jose";

import { Navbar } from "@/components/navbar";
import { siteConfig } from "@/config/site";
import { GenerixConfig, getGenerixConfig } from "@/authentication/providers/generix-config";

export default function DefaultLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const { t } = useTranslation();
  const { isAuthenticated, user, getAccessToken } = useAuth();
  const [accessToken, setAccessToken] = useState<string | null>(null);
  const [decodedToken, setDecodedToken] = useState<JWTPayload | null>(null);
  const [generixConfig, setGenerixConfig] = useState(null as GenerixConfig | null);

  useEffect(() => {
    const loadGenerixConfig = async () => {
      const config = await getGenerixConfig();
      console.log("Config is :", config);
      setGenerixConfig(config);
    };

    loadGenerixConfig();
  }, []);

  useEffect(() => {
    if (isAuthenticated && generixConfig) {
      getAccessToken().then((token) => {
        setAccessToken(token);
        const jwksUrl = `${generixConfig.authority}/.well-known/jwks.json`;
        console.log("JWKS URL:", jwksUrl);
        const JWKS = createRemoteJWKSet(
          new URL(
            jwksUrl,
          ),
        );
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
        <Link
          isExternal
          className="flex items-center gap-1 text-current"
          href={siteConfig().links.sctg}
          title={t("site-homepage")}
        >
          <span className="text-default-600">
            <Trans ns="base">powered-by</Trans>
          </span>
          <p className="text-primary">{t("brand")}&nbsp;</p>
        </Link>
        <Dropdown>
          <DropdownTrigger>
            {isAuthenticated ? (
              <span>
                {t("user")}: &nbsp;{user?.name}
              </span>
            ) : (
              <></>
            )}
          </DropdownTrigger>
          <DropdownMenu className="max-w-5xl">
            <DropdownItem key="user-logged" textValue="user-logged">
              <span className="text-default-600">{t("token")}:</span>
              <br />
              <Snippet className="max-w-4xl" symbol="" title="api-response">
                <div className="max-w-2xs sm:max-w-sm md:max-w-md lg:max-w-3xl  whitespace-break-spaces  text-wrap break-words">
                  {accessToken}
                </div>
              </Snippet>
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
            </DropdownItem>
          </DropdownMenu>
        </Dropdown>
        <Link className="flex items-center mx-1" color="secondary" href="/docs">
          API
        </Link>
      </footer>
    </div>
  );
}