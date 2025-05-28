import { Trans, useTranslation } from "react-i18next";

import { useEffect, useState } from "react";
import { Snippet } from "@heroui/snippet";

import { title } from "@/components/primitives";
import DefaultLayout from "@/layouts/default";
import { useAuth, useSecuredApi } from "@/authentication";
import { getGenerixConfig, GenerixConfig } from "../authentication/providers/generix-config";

export default function ApiPage() {
  const { t } = useTranslation();
  const { getJson } = useSecuredApi();
  const { user, isAuthenticated } = useAuth();
  const [apiResponse, setApiResponse] = useState("");
  const [generixConfig, setGenerixConfig] = useState(null as GenerixConfig | null);

  useEffect(() => {
    const loadGenerixConfig = async () => {
        const config = await getGenerixConfig();
        console.log("Config is :", config);
        setGenerixConfig(config);
    };
    
    loadGenerixConfig();
  }, []);

  // Call the API endpoint to get the response
  useEffect(() => {
    const fetchData = async () => {
      if (isAuthenticated && generixConfig && user) {
        try {
          
          const response = await getJson(
            `${generixConfig.api_base_url}/test/${user.sub}`,
          );
          setApiResponse(response);
        } catch (error) {
          setApiResponse((error as Error).message);
        }
      }
    };

    fetchData();
  }, [isAuthenticated, generixConfig, user?.username]);

  return (
    <DefaultLayout>
      <section className="flex flex-col items-center justify-center gap-4 py-8 md:py-10">
        <div className="inline-block max-w-lg text-center justify-center">
          <h1 className={title()}>
            <Trans t={t}>api-answer</Trans>
          </h1>
        </div>
        <Snippet className="max-w-11/12" symbol="" title="api-response">
          <div className="max-w-2xs sm:max-w-sm md:max-w-md lg:max-w-5xl  whitespace-break-spaces  text-wrap break-words">
            {JSON.stringify(apiResponse, null, 2)}
          </div>
        </Snippet>
      </section>
    </DefaultLayout>
  );
}
