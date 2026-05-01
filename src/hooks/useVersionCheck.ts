import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

interface VersionCheckResult {
  newVersion: string | null;
  releaseUrl: string | null;
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
  const [releaseUrl, setReleaseUrl] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function check() {
      // Test-mode override: set `localStorage.humos-test-update-banner` to a
      // version string from devtools (e.g. "9.9.9") to force the banner to
      // render against that fake version, bypassing fetch and dismiss state.
      // Lets you verify the banner UI before each release without shipping.
      // Clear it with `localStorage.removeItem("humos-test-update-banner")`.
      const testVersion = localStorage.getItem("humos-test-update-banner");
      if (testVersion && !cancelled) {
        const v = testVersion.replace(/^v/, "");
        setNewVersion(v);
        // Test-mode link points at /releases/latest, not a constructed
        // /tag/v<fake>. The fake version does not exist on GitHub, so a
        // tag-URL would 404 and confuse anyone clicking through during a
        // banner test. /latest always resolves to the real most-recent
        // release, which is what test-mode is meant to simulate.
        setReleaseUrl("https://github.com/humos-dev/humos/releases/latest");
        return;
      }

      try {
        const current = await getVersion();

        const controller = new AbortController();
        const timeout = setTimeout(() => controller.abort(), 3000);

        const res = await fetch(
          `https://humos.dev/version.json?v=${current}`,
          { signal: controller.signal }
        );
        clearTimeout(timeout);

        const data = await res.json();
        if (!cancelled && data?.version && semverGt(data.version, current)) {
          const dismissed = localStorage.getItem(`humos-dismissed-v${data.version}`);
          if (!dismissed) {
            setNewVersion(data.version);
            // Use the url from version.json. Always points to a real release.
            setReleaseUrl(data.url ?? `https://github.com/humos-dev/humos/releases/latest`);
          }
        }
      } catch (err) {
        // Surface the error in dev so a broken update check is visible.
        // Network errors, 3s timeouts, and parse failures all land here.
        console.warn("humOS update check failed:", err);
      }
    }

    check();
    return () => { cancelled = true; };
  }, []);

  return { newVersion, releaseUrl };
}
