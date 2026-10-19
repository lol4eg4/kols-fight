import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { registerPlugins } from "./plugin";
import type { RegisteredPlugin } from "./plugin";
import { Router } from "./router";
import { SHELL_SCREENS } from "./shell-screens";

/** Registers the plugins (checked against their manifests) and renders the app. Called once from web/src/main.tsx. */
export function mountApp(root: HTMLElement, plugins: RegisteredPlugin[]): void {
  registerPlugins(plugins, SHELL_SCREENS);
  createRoot(root).render(
    <StrictMode>
      <Router />
    </StrictMode>,
  );
}
