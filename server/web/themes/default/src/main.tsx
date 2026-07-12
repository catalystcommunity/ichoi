import { render } from "solid-js/web";
import "./styles.css";
import { App } from "./App.tsx";

const root = document.getElementById("root");
if (!root) throw new Error("Ichoi UI: #root element not found");

render(() => <App />, root);
