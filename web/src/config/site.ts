export type SiteConfig = typeof siteConfig;
import i18next from "../i18n";

export const siteConfig = () => ({
  name: i18next.t("vite-heroui"),
  needCookieConsent: true, // Set to false if you don't need cookie consent
  description: i18next.t(
    "make-beautiful-websites-regardless-of-your-design-experience",
  ),
  navItems: [
    {
      label: i18next.t("home"),
      href: "/",
    },
    {
      label: i18next.t("audio"),
      href: "/audio",
    },
    {
      label: i18next.t("thermal"),
      href: "/thermal",
    },
    {
      label: i18next.t("blog"),
      href: "/blog",
    },
    {
      label: i18next.t("graph"),
      href: "/graph",
    },
  ],
  navMenuItems: [
    {
      label: i18next.t("home"),
      href: "/",
    },
    {
      label: i18next.t("audio"),
      href: "/audio",
    },
    {
      label: i18next.t("thermal"),
      href: "/thermal",
    },
    {
      label: i18next.t("blog"),
      href: "/blog",
    },
    {
      label: i18next.t("graph"),
      href: "/graph",
    },
  ],
  links: {
    brand: "https://lasersmart.work",
    github: "https://github.com/sctg-development/rust-photoacoustic",
    twitter: "https://twitter.com/hero_ui",
    docs: "https://sctg-development.github.io/rust-photoacoustic/rust_photoacoustic/",
    discord: "https://discord.gg/9b6yyZKmH4",
    sponsor: "https://github.com/sponsors/sctg-development",
  },
});
