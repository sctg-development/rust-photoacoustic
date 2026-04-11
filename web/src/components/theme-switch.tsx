import { FC, useState, useEffect } from "react";
import { clsx } from "clsx";
import { useTranslation } from "react-i18next";

import { useTheme } from "../hooks/use-theme";
import { SunFilledIcon, MoonFilledIcon } from "../components/icons";

export interface ThemeSwitchProps {
  className?: string;
}

const themes = [
  { key: "light", Icon: MoonFilledIcon, i18nKey: "switch-to-dark-mode" },
  { key: "dark", Icon: SunFilledIcon, i18nKey: "switch-to-light-mode" },
] as const;

export const ThemeSwitch: FC<ThemeSwitchProps> = ({ className }) => {
  const { t } = useTranslation();
  const [isMounted, setIsMounted] = useState(false);
  const { theme, toggleTheme } = useTheme();

  useEffect(() => {
    setIsMounted(true);
  }, []);

  // Prevent Hydration Mismatch
  if (!isMounted) return <div className="w-6 h-6" />;

  const currentTheme = theme as "light" | "dark";

  return (
    <button
      aria-label={
        currentTheme === "light"
          ? t("switch-to-dark-mode")
          : t("switch-to-light-mode")
      }
      className={clsx(
        "inline-flex items-center justify-center",
        "rounded-md p-2 transition-colors",
        "text-default-500 hover:text-default-800",
        className,
      )}
      onClick={toggleTheme}
    >
      {themes.map(({ key, Icon }) => (
        <Icon
          key={key}
          aria-hidden="true"
          className={clsx(
            "w-5 h-5 transition-all absolute",
            currentTheme === key
              ? "opacity-100 scale-100"
              : "opacity-0 scale-75 pointer-events-none",
          )}
        />
      ))}
    </button>
  );
};
