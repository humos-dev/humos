import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

interface VersionCheckResult {
  newVersion: string | null;
}

function semverGt(a: string, b: string): boolean {
  const pa = a.replace(/^v/, "").split(".").map(Number);
  const pb = b.replace(/^v/, "").split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    if ((pa[i] ?? 0) > (pb[i] ?? 0)) return true;
    if ((pa[i] ?? 0) < (pb[i] ?? 0)) return false;
  }
  return false;
}

export function useVersionCheck(): VersionCheckResult {
  const [newVersion, setNewVersion] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function check() {
      try {
        const current = await getVersion();
        const dismissed = localStorage.getItem(`humos-dismissed-v${current}`);
        if (dismissed) return;

        const controller = new AbortController();
        const timeout = setTimeout(() => controller.abort(), 3000);

        const res = await fetch(
          `https://humos.dev/version.json?v=${current}`,
          { signal: controller.signal }
        );
        clearTimeout(timeout);

        const data = await res.json();
        if (!cancelled && data?.version && semverGt(data.version, current)) {
          setNewVersion(data.version);
        }
      } catch {
        // Network error, timeout, or parse failure — silently skip.
      }
    }

    check();
    return () => { cancelled = true; };
  }, []);

  return { newVersion };
}
