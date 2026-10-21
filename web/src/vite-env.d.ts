/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** "1" lets /team/design render without a session. Build-time only, off by default. See web/README.md. */
  readonly VITE_DESIGN_GALLERY_OPEN?: string;
}
