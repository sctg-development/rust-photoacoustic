/**
 * Copyright (c) 2024-2026 Ronan LE MEILLAT
 * License: AGPL-3.0-or-later
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of the
 * License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */

import { memo, useState, useEffect } from "react";
import { Modal } from "@heroui/react";
import { Chip } from "@heroui/react";
import { Separator } from "@heroui/react";
import { ScrollShadow } from "@heroui/react";
import { Button } from "@heroui/react";
import { Tooltip } from "@heroui/react";
import { Download } from "lucide-react";
import { JWTPayload } from "jose";
import { Trans, useTranslation } from "react-i18next";

import { CopyButton } from "./copy-button";
import { AuthUser } from "@/authentication/providers/auth-provider";
import { useTokenRefresh } from "../hooks/use-token-refresh";

/**
 * Props for the technical information modal displayed to authenticated users.
 */
interface UserTechnicalInfoModalProps {
  isOpen: boolean;
  onClose: () => void;
  user: AuthUser;
  accessToken: string | null;
  tokenPayload: JWTPayload | null;
  onTokenRefreshed?: (newToken: string) => Promise<void>;
}

/**
 * Format an Auth0 expiry timestamp as a localized French date string.
 * @param exp Unix timestamp in seconds
 * @returns Localized formatted expiration date string
 */
function formatExpiry(exp: number): string {
  return new Date(exp * 1000).toLocaleString("fr-FR", {
    day: "2-digit",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/**
 * Compute how many seconds remain until the token expires.
 * @param exp Expiration timestamp in seconds
 * @returns Remaining seconds until expiry, or 0 if already expired
 */
function getSecondsLeft(exp: number): number {
  return Math.max(0, Math.floor(exp - Date.now() / 1000));
}

/**
 * Format a duration from seconds into a human-readable string.
 * @param seconds Duration in seconds
 * @param t Translation function used for day pluralization
 * @returns Formatted duration string in HH:mm:ss or D days HH:mm:ss
 */
function formatDuration(seconds: number, t: any): string {
  const days = Math.floor(seconds / (24 * 3600));
  const hours = Math.floor((seconds % (24 * 3600)) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  const hms = [hours, minutes, secs]
    .map((v) => v.toString().padStart(2, "0"))
    .join(":");

  if (days > 0) {
    return `${t("duration-day", { count: days })} ${hms}`;
  }

  return hms;
}

/**
 * Export the current OIDC browser session into a Playwright storage state ZIP.
 * Captures localStorage keys from oidc-client-ts WebStorageStateStore
 * @param userEmail The email of the authenticated user
 * @param permissions Permission claims from the current token payload
 */
async function exportOauthPlaywrightSession(
  userEmail: string,
  permissions: string[],
): Promise<void> {
  const localStorageEntries: { name: string; value: string }[] = [];

  for (let i = 0; i < localStorage.length; i += 1) {
    const key = localStorage.key(i);
    if (!key) continue;
    // Capture OIDC-related keys from oidc-client-ts WebStorageStateStore
    if (
      key.startsWith("oidc.") ||
      key.startsWith("oidc:")
    ) {
      const value = localStorage.getItem(key);
      if (value !== null) {
        localStorageEntries.push({ name: key, value });
      }
    }
  }

  const cookieEntries = document.cookie
    .split(";")
    .map((cookie) => cookie.trim())
    .filter(Boolean)
    .map((cookie) => {
      const separatorIndex = cookie.indexOf("=");
      if (separatorIndex === -1) {
        return {
          name: cookie,
          value: "",
          domain: window.location.hostname,
          path: "/",
          secure: window.location.protocol === "https:",
          httpOnly: false,
          sameSite: "Lax" as const,
        };
      }

      return {
        name: cookie.substring(0, separatorIndex),
        value: cookie.substring(separatorIndex + 1),
        domain: window.location.hostname,
        path: "/",
        secure: window.location.protocol === "https:",
        httpOnly: false,
        sameSite: "Lax" as const,
      };
    });

  const storageState = {
    cookies: cookieEntries,
    origins: [
      {
        origin: window.location.origin,
        localStorage: localStorageEntries,
      },
    ],
  };

  const role = permissions.includes("admin") ? "admin" : "user";

  const readme = [
    "# OIDC/Generix Playwright Session",
    "",
    `**Email :** ${userEmail}`,
    `**Role :** ${role}`,
    `**Permissions :** ${permissions.join(", ") || "none"}`,
    `**Exported on :** ${new Date().toISOString()}`,
    "",
    "## Usage in Playwright",
    "",
    "```typescript",
    "// playwright.config.ts",
    "use: {",
    `  storageState: './e2e/fixtures/storage-state-${role}.json',`,
    "},",
    "```",
    "",
    "```typescript",
    "// In a specific test",
    `test.use({ storageState: './e2e/fixtures/storage-state-${role}.json' });`,
    "```",
    "",
    "> ⚠️  This file contains OIDC authentication tokens.",
    "> Do not commit this file to the repository.",
    "> Add `e2e/fixtures/storage-state-*.json` to `.gitignore`.",
  ].join("\n");

  const JSZip = (await import("jszip")).default;
  const zip = new JSZip();
  zip.file("storage-state.json", JSON.stringify(storageState, null, 2));
  zip.file("README.md", readme);

  const blob = await zip.generateAsync({ type: "blob" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  const safeEmail = userEmail.replace(/[^a-z0-9]/gi, "-").toLowerCase();
  a.href = url;
  a.download = `oidc-session-${safeEmail}-${role}.zip`;
  a.click();
  URL.revokeObjectURL(url);
}

/**
 * Modal component exposing user technical info, token status, and E2E export utilities.
 * It displays Auth0 profile info, JWT claims, token expiry, and an E2E export action.
 * Intended for admin / debug use only; render only with the appropriate permission guard.
 */
export const UserTechnicalInfoModal = memo<UserTechnicalInfoModalProps>(
  ({ isOpen, onClose, user, accessToken, tokenPayload, onTokenRefreshed }) => {
    const { t } = useTranslation();
    const { refreshToken } = useTokenRefresh({ onTokenRefreshed });
    const [secondsLeft, setSecondsLeft] = useState<number>(0);
    const [isRefreshing, setIsRefreshing] = useState<boolean>(false);
    const [refreshError, setRefreshError] = useState<string | null>(null);
    const [isModalOpen, setIsModalOpen] = useState(false);

    // Sync parent isOpen prop with local state
    useEffect(() => {
      setIsModalOpen(isOpen);
    }, [isOpen]);

    const handleOpenChange = (open: boolean) => {
      setIsModalOpen(open);
      if (!open && onClose) {
        onClose();
      }
    };

    useEffect(() => {
      if (!tokenPayload?.exp) return;
      setSecondsLeft(getSecondsLeft(tokenPayload.exp));
      const interval = setInterval(() => {
        setSecondsLeft(getSecondsLeft(tokenPayload.exp!));
      }, 1000);

      return () => clearInterval(interval);
    }, [tokenPayload?.exp]);

    const handleRefreshToken = async () => {
      setIsRefreshing(true);
      setRefreshError(null);
      try {
        await refreshToken();
      } catch (error) {
        console.error("[Modal] Token refresh error:", error);
        setRefreshError(t("nav-user-dropdown-refresh-token-failed"));
      } finally {
        setIsRefreshing(false);
      }
    };

    const permissions =
      (tokenPayload?.permissions as string[] | undefined) ?? [];
    const isExpiringSoon = secondsLeft < 120;

    return (
      <Modal isOpen={isModalOpen} onOpenChange={handleOpenChange}>
        <Modal.Backdrop>
          <Modal.Container placement="bottom">
            <Modal.Dialog>
              {({ close }) => (
                <>
                  <Modal.CloseTrigger onPress={close} />
                  <Modal.Header className="flex items-center gap-2 border-b border-default-100 pb-3">
                    <div className="flex flex-col gap-0.5">
                      <span className="font-black text-foreground text-base leading-tight">
                        {t("nav-user-dropdown-connected-as")}
                      </span>
                      <span className="text-sm font-semibold text-primary truncate max-w-75">
                        {user.email}
                      </span>
                    </div>
                  </Modal.Header>

                  <Modal.Body className="px-5 py-4 gap-2">
                    {/* User Identity */}
                    <div className="bg-default-100 border border-default-200 rounded-xl p-3 space-y-1">
                      <p className="text-xs text-default-400 uppercase tracking-wider font-bold mb-2">
                        Identité
                      </p>
                      <p className="text-sm font-semibold text-foreground">
                        {user.name}
                      </p>
                      <p className="text-xs text-default-500  break-all">
                        ID: {user.sub}
                      </p>
                    </div>

                    <Separator className="my-2" />

                    {/* Token Status */}
                    <div className="space-y-2">
                      <div className="flex items-center justify-between">
                        <p className="text-xs text-default-400 uppercase tracking-wider font-bold">
                          {t("nav-user-dropdown-token-status")}
                        </p>
                        <Tooltip>
                          <Tooltip.Trigger>
                            <Button
                              isIconOnly
                              className="h-7 w-7 min-w-7 bg-default text-foreground"
                              isDisabled={isRefreshing}
                              size="sm"
                              onPress={() => {
                                console.log("[Modal] Button pressed");
                                handleRefreshToken();
                              }}
                            >
                              <svg
                                className="w-4 h-4"
                                fill="none"
                                stroke="currentColor"
                                viewBox="0 0 24 24"
                              >
                                <path
                                  d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
                                  strokeLinecap="round"
                                  strokeLinejoin="round"
                                  strokeWidth={2}
                                />
                              </svg>
                            </Button>
                          </Tooltip.Trigger>
                          <Tooltip.Content>
                            {t("nav-user-dropdown-refresh-token")}
                          </Tooltip.Content>
                        </Tooltip>
                      </div>
                      {tokenPayload?.exp ? (
                        <div className="space-y-2">
                          <div className="flex justify-between items-center text-sm">
                            <span className="text-default-500">
                              {t("nav-user-dropdown-expires-in")}
                            </span>
                            <Chip
                              className=" font-bold text-xs"
                              color={isExpiringSoon ? "danger" : "success"}
                              size="sm"
                            >
                              {formatDuration(secondsLeft, t)}
                            </Chip>
                          </div>
                          <div className="flex justify-between items-center text-xs text-default-500 my-2">
                            <span>Expiration</span>
                            <span className=" text-default-400">
                              {formatExpiry(tokenPayload.exp)}
                            </span>
                          </div>
                        </div>
                      ) : (
                        <p className="text-xs text-danger">
                          {t("nav-user-dropdown-no-expiry")}
                        </p>
                      )}
                      {refreshError && (
                        <p className="text-xs text-danger mt-1">
                          {refreshError}
                        </p>
                      )}
                    </div>

                    {/* Permissions */}
                    {permissions.length > 0 && (
                      <>
                        <Separator className="my-2" />
                        <div className="space-y-2">
                          <p className="text-xs text-default-400 uppercase tracking-wider font-bold">
                            Permissions
                          </p>
                          <div className="flex flex-wrap gap-1.5">
                            {permissions.map((perm) => {
                              const isAdminPermission =
                                import.meta.env.ADMIN_AUTH0_PERMISSION &&
                                perm ===
                                  (import.meta.env
                                    .ADMIN_AUTH0_PERMISSION as string);

                              return (
                                <Chip
                                  key={perm}
                                  className="text-xs"
                                  color={isAdminPermission ? "accent" : "default"}
                                  size="sm"
                                >
                                  {perm}
                                  {isAdminPermission && " (Admin)"}
                                </Chip>
                              );
                            })}
                          </div>
                        </div>
                      </>
                    )}

                    <Separator className="my-2" />

                    {/* Playwright session export for E2E debugging */}
                    <div className="space-y-2">
                      <div className="flex items-center justify-between">
                        <p className="text-xs text-default-400 uppercase tracking-wider font-bold">
                          Playwright
                        </p>
                        <Tooltip>
                          <Tooltip.Trigger>
                            <Button
                              isIconOnly
                              className="h-7 w-7 min-w-7 rounded-full bg-warning/10 text-warning-600 hover:bg-warning-soft-hover"
                              size="sm"
                              onPress={() =>
                                exportOauthPlaywrightSession(
                                  user.email ?? "",
                                  permissions,
                                )
                              }
                            >
                              <Download className="w-4 h-4 bg-yellow-200 text-gray-700" />
                            </Button>
                          </Tooltip.Trigger>
                          <Tooltip.Content>
                            Export Playwright session (.zip)
                          </Tooltip.Content>
                        </Tooltip>
                      </div>
                      <p className="text-xs text-default-400 leading-relaxed">
                        <Trans
                          ns="base"
                          i18nKey="nav-user-dropdown-playwright-export-description"
                          components={[
                            <code key="0" className="font-mono text-xs" />,
                          ]}
                        />
                      </p>
                    </div>

                    <Separator className="my-2" />

                    {/* Access Token */}
                    <div className="space-y-2">
                      <div className="flex items-center justify-between">
                        <p className="text-xs text-default-400 uppercase tracking-wider font-bold">
                          {t("nav-user-dropdown-access-token")}
                        </p>
                        <CopyButton
                          className="h-7 w-7 min-w-7 bg-gray-50"
                          value={accessToken ?? ""}
                        />
                      </div>
                      <ScrollShadow
                        className="h-32 w-full"
                        orientation="horizontal"
                      >
                        <p className="text-[10px] text-default-500  break-all leading-relaxed select-all">
                          {accessToken || t("nav-user-dropdown-loading")}
                        </p>
                      </ScrollShadow>
                    </div>
                  </Modal.Body>
                </>
              )}
            </Modal.Dialog>
          </Modal.Container>
        </Modal.Backdrop>
      </Modal>
    );
  },
);

UserTechnicalInfoModal.displayName = "UserTechnicalInfoModal";
