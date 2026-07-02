import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "@fontsource-variable/newsreader/index.css";
import "@fontsource-variable/newsreader/wght-italic.css";
import "@fontsource-variable/inter/index.css";
import "@fontsource-variable/jetbrains-mono/index.css";
import "./styles.css";

createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
