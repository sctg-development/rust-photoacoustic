import { Route, Routes } from "react-router-dom";
import { Suspense } from "react";
import { useTranslation } from "react-i18next";

import { SiteLoading } from "./components/site-loading";
import DefaultLayout from "./layouts/default";
import { title } from "./components/primitives";
import { PageNotFound } from "./pages/404";
import { AuthenticationGuard, LogoutButton, useAuth } from "./authentication";

import IndexPage from "@/pages/index";
import AudioPage from "@/pages/audio";
import ThermalPage from "@/pages/thermal";
import BlogPage from "@/pages/blog";
import GraphPage from "@/pages/graph";

function App() {
  const { isLoading, isAuthenticated } = useAuth();
  const { t } = useTranslation();

  if (isLoading) {
    return <SiteLoading />;
  }

  // Gérer les erreurs
  if (!isAuthenticated && !isLoading) {
    // eslint-disable-next-line no-console
    console.log(
      "User is not authenticated but auth is not loading - likely an error condition"
    );

    return (
      <DefaultLayout>
        <section className="flex flex-col items-center justify-center gap-4 py-8 md:py-10">
          <div className="inline-block max-w-lg text-center justify-center">
            <div className="mb-4">
              <h1 className={title()}>
                {t("error")}: {t("authentication_error")}
              </h1>
            </div>
            <div>
              <LogoutButton
                showButtonIfNotAuthenticated={true}
                text={t("reload")}
              />
            </div>
          </div>
        </section>
      </DefaultLayout>
    );
  }

  return (
    <Suspense fallback={<SiteLoading />}>
      <Routes>
        <Route element={<IndexPage />} path="/" />
        <Route
          element={<AuthenticationGuard component={AudioPage} />}
          path="/audio"
        />
        <Route
          element={<AuthenticationGuard component={ThermalPage} />}
          path="/thermal"
        />
        <Route
          element={<AuthenticationGuard component={BlogPage} />}
          path="/blog"
        />
        <Route
          element={<AuthenticationGuard component={GraphPage} />}
          path="/graph"
        />
        <Route element={<PageNotFound />} path="*" />
      </Routes>
    </Suspense>
  );
}

export default App;
