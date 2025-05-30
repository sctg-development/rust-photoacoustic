import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";

import App from "./App.tsx";
import "./i18n";
import { Provider } from "./provider.tsx";
import "@/styles/globals.css";
import { CookieConsentProvider } from "./contexts/cookie-consent-context.tsx";
import { CookieConsent } from "./components/cookie-consent.tsx";
import { AuthenticationProvider } from "./authentication";
import { getGenerixConfig } from "./authentication/providers/generix-config";

import type { AuthenticationType } from "./authentication";

const root = document.getElementById("root");

(async () => {
  let providerType: AuthenticationType = "auth0";
  let generixConfig = null;

  try {
    generixConfig = await getGenerixConfig();
    providerType = (generixConfig?.provider as AuthenticationType) || "auth0";
    ReactDOM.createRoot(root!).render(
      <React.StrictMode>
        <BrowserRouter basename={import.meta.env.BASE_URL}>
          <Provider>
            <CookieConsentProvider>
              <AuthenticationProvider
                config={generixConfig}
                providerType={providerType}
              >
                <CookieConsent />
                <App />
              </AuthenticationProvider>
            </CookieConsentProvider>
          </Provider>
        </BrowserRouter>
      </React.StrictMode>,
    );
  } catch (e) {
    providerType =
      (import.meta.env.AUTHENTICATION_PROVIDER_TYPE as AuthenticationType) ||
      "auth0";
  }
})();
