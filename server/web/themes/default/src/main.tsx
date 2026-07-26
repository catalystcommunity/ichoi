import { render } from "solid-js/web";
import "./styles.css";
import { App } from "./App.tsx";
import { finishUpdateReload, updateReloadInProgress } from "./lib/app-update.ts";

const root = document.getElementById("root");
if (!root) throw new Error("Ichoi UI: #root element not found");

render(() => <App />, root);

// Keep the update-reload marker long enough for the satellite to reclaim its output and resume
// the server-backed track. It suppresses the old document's unload-induced pause report.
if (updateReloadInProgress()) {
  setTimeout(finishUpdateReload, 5_000);
}

if ("serviceWorker" in navigator && import.meta.env.PROD) {
  void navigator.serviceWorker.register("/sw.js");
}
