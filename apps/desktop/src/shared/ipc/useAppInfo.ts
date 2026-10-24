/**
 * React hook: loads `AppInfo` from the Rust core once.
 * Why a hook: several places (footer now, "Report a problem" later) need the
 * same data with the same loading/error states.
 */
import { useEffect, useState } from "react";

import { commands } from "./commands";
import type { AppError } from "./generated/AppError";
import type { AppInfo } from "./generated/AppInfo";

export type AppInfoState =
  | { status: "loading" }
  | { status: "ready"; value: AppInfo }
  | { status: "error"; error: AppError };

export function useAppInfo(): AppInfoState {
  const [state, setState] = useState<AppInfoState>({ status: "loading" });

  useEffect(() => {
    let alive = true; // ignore the result if the component unmounted meanwhile
    void commands.appInfo().then((result) => {
      if (!alive) return;
      setState(
        result.ok
          ? { status: "ready", value: result.value }
          : { status: "error", error: result.error },
      );
    });
    return () => {
      alive = false;
    };
  }, []);

  return state;
}
