import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { applyTheme, readTheme } from "./store/themeEffects";
import { App } from "./views/App";
import "./tokens.css";
import "./style.css";
import "./chain.css";
import "./panel.css";
import "./threads.css";
import "./settings.css";
import "./usage.css";
import "./plan.css";
import "./menu.css";
import "./updates.css";
import "./inbox.css";
import "./details.css";
import "./assistant.css";

// The viewer's theme goes on before the first paint, so a forced dark or light does not flash.
applyTheme(readTheme());

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
