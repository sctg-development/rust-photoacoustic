import type React from "react";

import { Toast } from "@heroui/react";

export function Provider({ children }: { children: React.ReactNode }) {
  return (
    <>
      <Toast.Provider />
      {children}
    </>
  );
}
