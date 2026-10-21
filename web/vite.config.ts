import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  // One copy of React for the shell, the design package and every plugin frontend.
  resolve: { dedupe: ["react", "react-dom"] },
  build: {
    // The bundle check reads which chunks a client page loads from this manifest.
    manifest: true,
  },
});
