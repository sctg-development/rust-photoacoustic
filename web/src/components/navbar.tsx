import { useState } from "react";
import { Trans, useTranslation } from "react-i18next";

import { LoginLogoutButton } from "../authentication";
import { siteConfig } from "../config/site";
import { ThemeSwitch } from "../components/theme-switch";
import {
  GithubIcon,
  HeartFilledIcon,
  LaserIcon,
  SearchIcon,
} from "../components/icons";
import { availableLanguages } from "../i18n";

import { I18nIcon, LanguageSwitch } from "./language-switch";
import { LinkUniversal } from "./link-universal";

export const Navbar = () => {
  const { t } = useTranslation();
  const [isMenuOpen, setIsMenuOpen] = useState(false);

  return (
    <header className="sticky top-0 z-50 border-b border-default-100 bg-background/95 backdrop-blur">
      <div className="mx-auto flex max-w-7xl items-center justify-between gap-4 px-4 py-2">
        {/* Brand */}
        <div className="flex items-center gap-4">
          <a className="flex items-center gap-1 text-foreground" href="/">
            <LaserIcon />
            <p className="font-bold text-inherit">LaserSmart</p>
          </a>
          <div className="hidden lg:flex items-center gap-3">
            {siteConfig().navItems.map((item) => (
              <LinkUniversal
                key={item.href}
                className="text-default-700 hover:text-primary transition-colors text-sm"
                href={item.href}
              >
                {item.label}
              </LinkUniversal>
            ))}
          </div>
        </div>

        {/* Desktop actions */}
        <div className="hidden sm:flex items-center gap-2">
          <div className="hidden lg:flex items-center">
            <div className="relative">
              <SearchIcon className="absolute left-2 top-1/2 -translate-y-1/2 text-default-400 pointer-events-none" />
              <input
                aria-label={t("search")}
                className="bg-default-100 text-sm rounded-md pl-8 pr-3 py-1.5 outline-none focus:ring-1 focus:ring-primary"
                placeholder={`${t("search")}…`}
                type="search"
              />
            </div>
          </div>
          <a
            href={siteConfig().links.github}
            rel="noopener noreferrer"
            target="_blank"
            title={t("github")}
          >
            <GithubIcon className="text-default-500" />
          </a>
          <ThemeSwitch />
          <LanguageSwitch
            availableLanguages={availableLanguages}
            icon={I18nIcon}
          />
          <LoginLogoutButton />
          <a
            className="flex items-center gap-1 text-sm text-default-600 bg-default-100 rounded-md px-3 py-1.5 hover:bg-default-200 transition-colors"
            href={siteConfig().links.sponsor}
            rel="noopener noreferrer"
            target="_blank"
          >
            <HeartFilledIcon className="text-danger" />
            <Trans i18nKey="sponsor" />
          </a>
        </div>

        {/* Mobile actions */}
        <div className="flex items-center gap-2 sm:hidden">
          <a
            href={siteConfig().links.github}
            rel="noopener noreferrer"
            target="_blank"
          >
            <GithubIcon className="text-default-500" />
          </a>
          <ThemeSwitch />
          <button
            aria-label="Toggle menu"
            className="p-2 text-default-500"
            onClick={() => setIsMenuOpen(!isMenuOpen)}
          >
            <svg
              className="w-5 h-5"
              fill="none"
              stroke="currentColor"
              strokeWidth={2}
              viewBox="0 0 24 24"
            >
              {isMenuOpen ? (
                <path
                  d="M6 18L18 6M6 6l12 12"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              ) : (
                <path
                  d="M4 6h16M4 12h16M4 18h16"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              )}
            </svg>
          </button>
        </div>
      </div>

      {/* Mobile menu */}
      {isMenuOpen && (
        <div className="sm:hidden border-t border-default-100 bg-background px-4 py-3 flex flex-col gap-3">
          <div className="relative">
            <SearchIcon className="absolute left-2 top-1/2 -translate-y-1/2 text-default-400 pointer-events-none" />
            <input
              aria-label={t("search")}
              className="w-full bg-default-100 text-sm rounded-md pl-8 pr-3 py-1.5 outline-none focus:ring-1 focus:ring-primary"
              placeholder={`${t("search")}…`}
              type="search"
            />
          </div>
          <LanguageSwitch
            availableLanguages={availableLanguages}
            icon={I18nIcon}
          />
          <div className="flex flex-col gap-2">
            {siteConfig().navMenuItems.map((item, index) => (
              <a
                key={`${item}-${index}`}
                className={
                  index === 2
                    ? "text-primary text-lg"
                    : index === siteConfig().navMenuItems.length - 1
                      ? "text-danger text-lg"
                      : "text-foreground text-lg"
                }
                href={item.href}
              >
                {item.label}
              </a>
            ))}
            <LoginLogoutButton />
          </div>
        </div>
      )}
    </header>
  );
};
