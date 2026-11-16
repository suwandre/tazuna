import { invoke } from "@tauri-apps/api/core";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { inTauri } from "./lib/tazuna";
import "./styles.css";

// the page says it runs, and any script error, on the app's stderr (a terminal, a CI log): a blank window is otherwise
// silent
if (inTauri) {
  const log = (msg: string) => invoke("ui_log", { msg }).catch(() => {});
  window.addEventListener("error", (e) => log(`error: ${e.message} (${e.filename}:${e.lineno})`));
  window.addEventListener("unhandledrejection", (e) => log(`unhandled: ${String(e.reason)}`));
  log("loaded");
}

createRoot(document.getElementById("root")!).render(<StrictMode><App /></StrictMode>);
