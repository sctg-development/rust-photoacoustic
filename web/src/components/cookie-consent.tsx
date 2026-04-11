import { Button, Link } from "@heroui/react";
import React from "react";
import { Trans, useTranslation } from "react-i18next";

import { useCookieConsent } from "../contexts/cookie-consent-context";
import { siteConfig } from "../config/site";

import { buttonGradient } from "./primitives";

export const CookieConsent: React.FC = () => {
  const { t } = useTranslation();
  const { cookieConsent, acceptCookies, rejectCookies } = useCookieConsent();

  const isOpen = cookieConsent === "pending" && siteConfig().needCookieConsent;

  if (!isOpen) {
    return null;
  }

  return (
    <div className="fixed inset-0 z-50 flex items-end justify-center bg-black/40 p-4">
      <div className="w-full max-w-lg rounded-xl bg-white p-4 shadow-2xl dark:bg-default-100">
        <div className="text-lg font-semibold text-default-900">
          {t("cookie-consent-title")}
        </div>
        <div className="mt-2 text-sm font-normal text-default-700">
          <Trans i18nKey="cookie-consent" t={t} />
          &nbsp;
          <Link className="text-sm" href="#">
            {t("cookie-policy")}
          </Link>
        </div>
        <div className="mt-4 flex justify-end gap-2">
          <div className="mt-4 flex items-center gap-x-1">
            <Button
              className={buttonGradient({ bordered: "violet" })}
              onPress={acceptCookies}
            >
              {t("accept-all")}
            </Button>
            <Button
              className="rounded-large"
              variant="secondary"
              onPress={rejectCookies}
            >
              {t("reject")}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
};
