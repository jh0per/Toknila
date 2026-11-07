import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./views/App";
import "./tokens.css";
import "./style.css";
import "./chain.css";
import "./panel.css";
import "./threads.css";
import "./settings.css";
import "./usage.css";
import "./updates.css";

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
