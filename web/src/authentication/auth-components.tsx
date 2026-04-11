/**
 * @copyright Copyright (c) 2024-2025 Ronan LE MEILLAT
 * @license AGPL-3.0-or-later
 */

import { FC, ReactNode, useEffect, useState } from "react";
import { Button, Tooltip, Link } from "@heroui/react";
import { useTranslation } from "react-i18next";

import { SiteLoading } from "../components/site-loading";

import {
  useAuth,
  getNameWithFallback,
  withAuthentication,
} from "./providers/use-auth";

/**
 * Renders the user's profile name with a tooltip showing their username.
 * @returns The user's name with a tooltip showing their username
 */
export function Profile() {
  const { user } = useAuth();

  console.log(JSON.stringify(user));

  return (
    <Tooltip delay={750}>
      <Tooltip.Trigger>
        <span>{user?.name}</span>
      </Tooltip.Trigger>
      <Tooltip.Content>
        <p>{user?.nickname || ""}</p>
      </Tooltip.Content>
    </Tooltip>
  );
}

/**
 * Renders a login button for authentication.
 * Only shows when the user is not authenticated.
 * @param props - Component props
 * @param [props.text] - Custom text for the button. Defaults to localized "log-in" text.
 * @returns Login button or null if user is already authenticated
 */
export const LoginButton: FC<{ text?: string }> = ({ text }) => {
  const { isAuthenticated, login } = useAuth();
  const { t } = useTranslation();

  if (!text) {
    text = t("log-in");
  }

  return (
    !isAuthenticated && (
      <Button
        className="text-sm font-normal text-default-600 bg-default-100"
        type="button"
        onPress={() => login()}
      >
        {text}
      </Button>
    )
  );
};

/**
 * Renders a login link for authentication.
 * Only shows when the user is not authenticated.
 */
export const LoginLink: FC<{
  text?: string;
}> = ({ text }) => {
  const { isAuthenticated, login } = useAuth();
  const { t } = useTranslation();

  if (!text) {
    text = t("log-in");
  }

  return (
    !isAuthenticated && (
      <Link
        onPress={() => {
          login();
        }}
      >
        {text}
      </Link>
    )
  );
};

interface LogoutButtonProps {
  showButtonIfNotAuthenticated?: boolean;
  text?: string;
}

/**
 * Renders a logout button for authentication.
 */
export const LogoutButton: FC<LogoutButtonProps> = ({
  showButtonIfNotAuthenticated = false,
  text,
}) => {
  const { isAuthenticated, logout, user } = useAuth();
  const { t } = useTranslation();

  if (!text) {
    text = t("log-out-someone", {
      name: getNameWithFallback(user),
    });
  }

  return (
    (isAuthenticated || showButtonIfNotAuthenticated) && (
      <Tooltip delay={750}>
        <Tooltip.Trigger>
          <Button
            className="text-sm font-normal text-default-600 bg-default-100"
            type="button"
            onPress={() => {
              logout({
                logoutParams: {
                  returnTo: new URL(
                    import.meta.env.BASE_URL || "/",
                    window.location.origin,
                  ).toString(),
                },
              });
            }}
          >
            <span>{text}</span>
          </Button>
        </Tooltip.Trigger>
        <Tooltip.Content>
          <p>{user?.name || ""}</p>
          <p>{user?.nickname || ""}</p>
          <p>{user?.email || ""}</p>
          <p>{user?.sub || ""}</p>
        </Tooltip.Content>
      </Tooltip>
    )
  );
};

interface LogoutLinkProps extends LogoutButtonProps {}

/**
 * Renders a logout link for authentication.
 */
export const LogoutLink: FC<LogoutLinkProps> = ({
  showButtonIfNotAuthenticated = false,
  text,
}) => {
  const { isAuthenticated, logout, user } = useAuth();
  const { t } = useTranslation();

  if (!text) {
    text = t("log-out-someone", {
      name: getNameWithFallback(user),
    });
  }

  return isAuthenticated || showButtonIfNotAuthenticated ? (
    <>
      <Link
        onPress={() => {
          logout({
            logoutParams: {
              returnTo: new URL(
                import.meta.env.BASE_URL || "/",
                window.location.origin,
              ).toString(),
            },
          });
        }}
      >
        {text}
      </Link>
    </>
  ) : null;
};

/**
 * Conditionally renders either a login or logout button based on authentication status.
 */
export const LoginLogoutButton: FC<LogoutButtonProps> = ({
  showButtonIfNotAuthenticated = false,
  text,
}) => {
  const { isAuthenticated } = useAuth();

  return isAuthenticated ? (
    <LogoutButton
      showButtonIfNotAuthenticated={showButtonIfNotAuthenticated}
      text={text}
    />
  ) : (
    <LoginButton />
  );
};

/**
 * Conditionally renders either a login or logout link based on authentication status.
 */
export const LoginLogoutLink: FC<LogoutLinkProps> = ({
  showButtonIfNotAuthenticated = false,
  text,
}) => {
  const { isAuthenticated } = useAuth();

  return isAuthenticated ? (
    <LogoutLink
      showButtonIfNotAuthenticated={showButtonIfNotAuthenticated}
      text={text}
    />
  ) : (
    <LoginLink />
  );
};

/**
 * Higher-order component that protects routes requiring authentication.
 */
export const AuthenticationGuard: FC<{ component: FC }> = ({ component }) => {
  const Component = withAuthentication(component, {
    onRedirecting: () => <SiteLoading />,
  });

  return <Component />;
};

/**
 * Component that conditionally renders its children based on whether the user has a specific permission.
 */
export const AuthenticationGuardWithPermission: FC<{
  permission: string;
  children: ReactNode;
  fallback?: ReactNode;
}> = ({ permission, children, fallback = <></> }) => {
  const { hasPermission } = useAuth();
  const [permitted, setPermitted] = useState<boolean | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let isMounted = true;

    const checkPermission = async () => {
      try {
        const result = await hasPermission(permission);

        if (isMounted) {
          setPermitted(result);
          setIsLoading(false);
        }
      } catch (error) {
        console.error("Permission check failed:", error);
        if (isMounted) {
          setPermitted(false);
          setIsLoading(false);
        }
      }
    };

    checkPermission();

    return () => {
      isMounted = false;
    };
  }, [permission, hasPermission]);

  if (isLoading) {
    return <SiteLoading />;
  }

  return permitted ? <>{children}</> : <>{fallback}</>;
};

/**
 * Custom hook that provides secured API fetching capabilities.
 */
export const useSecuredApi = () => {
  const { getJson, postJson, deleteJson, hasPermission } = useAuth();

  return {
    getJson,
    postJson,
    deleteJson,
    hasPermission,
  };
};
