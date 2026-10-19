// @arena/shell: what plugin frontends (and the app entry) may use from the web shell.
// Importing this module registers the shell's own dictionary.

import { dict } from "./en";
import { addDictionary } from "./i18n";

addDictionary("shell", dict);

// Plugins
export { definePlugin, registerPlugins, getRegistry } from "./plugin";
export type { Plugin, PluginDefinition, RegisteredPlugin, ScreenEntry, ScreenLoader, SlotFill, WidgetFill, Registry } from "./plugin";
export type { PluginManifest, ScreenManifest, Tag, SlotName, SlotProps, SlotPropsMap, WidgetName, WidgetProps, WidgetPropsMap } from "./contract";
export { SLOT_NAMES, WIDGET_NAMES } from "./contract";

// Slots, widgets, ticker
export { Slot, Widget, ErrorBoundary } from "./slots";
export { useTickerItems } from "./frame";

// Page events
export { emitPageEvent, onPageEvent, usePageEvent, PAGE_EVENT_NAMES } from "./events";
export type { PageEventMap, PageEventName } from "./events";

// Text
export { t, hasKey } from "./i18n";

// Server calls
export { api, useApi } from "./api";
export type { ApiResult, ApiError, ApiInit, ApiState } from "./api";

// Navigation
export { Link, navigate, useLocation, useParams, useSearchParams, useOverlayClose, closeOverlay } from "./nav";
export type { LinkProps, Location } from "./nav";
export { ANCHORS, safeNext, LOGIN_PATH, TEAM_HOME } from "./route";

// Session (closed section)
export { useSession, signOut } from "./session";
export type { Person, SessionState } from "./session";

// App entry
export { mountApp } from "./app";
