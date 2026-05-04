import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export function DaemonLoginBanner() {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    invoke<boolean>("check_login_item_banner")
      .then((shouldShow) => { if (shouldShow) setVisible(true); })
      .catch(() => {});
  }, []);

  if (!visible) return null;

  return (
    <div className="update-banner">
      <div className="update-banner__left">
        <span className="update-banner__arrow">&#x21BA;</span>
        <span>Daemon set to start at login.</span>
      </div>
      <div className="update-banner__right">
        <span style={{ fontSize: "0.78rem", opacity: 0.7, marginRight: "0.75rem" }}>
          Project Brain ribbon now persists across sessions.
        </span>
        <button
          className="update-banner__copy"
          onClick={() => setVisible(false)}
          aria-label="Dismiss"
        >
          Got it
        </button>
      </div>
    </div>
  );
}
