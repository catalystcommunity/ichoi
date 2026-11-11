import { defineControls } from "@catalystcommunity/controls";
import { defineContainers } from "@catalystcommunity/containers";
import { defineMenus } from "@catalystcommunity/menus";
import { defineDataViews } from "@catalystcommunity/data-views";
import "./styles.css";
import { finishUpdateReload, updateReloadInProgress } from "./lib/app-update.ts";

defineControls(); defineContainers(); defineMenus(); defineDataViews();
const { IchoiApp } = await import("./app.ts");
const root = document.getElementById("root");
if (!root) throw new Error("Ichoi UI: #root element not found");
root.replaceChildren(new IchoiApp());
if (updateReloadInProgress()) setTimeout(finishUpdateReload, 5_000);
if ("serviceWorker" in navigator && import.meta.env.PROD) void navigator.serviceWorker.register("/sw.js");
