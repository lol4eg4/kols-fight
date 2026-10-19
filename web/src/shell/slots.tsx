import { Component, Suspense } from "react";
import type { ComponentType, ErrorInfo, ReactNode } from "react";
import { ErrorLine, Skeleton } from "@arena/design";
import type { SlotName, SlotProps, WidgetName, WidgetProps } from "./contract";
import { t } from "./i18n";
import { getRegistry } from "./plugin";

interface BoundaryProps {
  children: ReactNode;
  /** Shown instead of the children after a render error; gets a way to try again. */
  fallback: (retry: () => void) => ReactNode;
}

/** A plugin that fails to render takes down only its own slot, screen or widget, never the page around it. */
export class ErrorBoundary extends Component<BoundaryProps, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("render failed", error, info.componentStack);
  }

  render() {
    return this.state.failed ? this.props.fallback(() => this.setState({ failed: false })) : this.props.children;
  }
}

function Guarded({ children, loading }: { children: ReactNode; loading?: ReactNode }) {
  return (
    <ErrorBoundary fallback={(retry) => <ErrorLine message={t("shell.error.render")} onRetry={retry} retryLabel={t("shell.retry")} />}>
      <Suspense fallback={loading === undefined ? <Skeleton label={t("shell.loading")} /> : loading}>{children}</Suspense>
    </ErrorBoundary>
  );
}

export interface SlotComponentProps<K extends SlotName> {
  /** "<host plugin>/<slot>", for example "board/rank_change". */
  name: K;
  /** Ids and dates only. The filler loads its own data. */
  props: SlotProps<K>;
  /** What to show while a lazy filler loads. Default: a Skeleton. Pass null for nothing. */
  loading?: ReactNode;
  /** The slot's default value: shown when no plugin fills the slot. */
  children?: ReactNode;
}

/** A named place a plugin leaves open. Fillers are shown in layer order; with no filler the slot shows its default. */
export function Slot<K extends SlotName>({ name, props, loading, children }: SlotComponentProps<K>) {
  const fills = getRegistry().fills.get(name);
  if (!fills || fills.length === 0) return <>{children}</>;
  return (
    <>
      {fills.map(({ plugin, component }) => {
        const Filler = component as unknown as ComponentType<SlotProps<K>>;
        return (
          <Guarded key={plugin} loading={loading}>
            <Filler {...props} />
          </Guarded>
        );
      })}
    </>
  );
}

export interface WidgetComponentProps<K extends WidgetName> {
  /** "<owner plugin>/<widget>", for example "fight/add_to_fight". */
  name: K;
  props: WidgetProps<K>;
  /** Shown when the owner plugin is off or has not built the widget. Default: nothing. */
  fallback?: ReactNode;
  loading?: ReactNode;
}

/** A piece of screen another plugin offers. The widget loads its own data from the ids in props. */
export function Widget<K extends WidgetName>({ name, props, fallback = null, loading }: WidgetComponentProps<K>) {
  const entry = getRegistry().widgets.get(name);
  if (!entry) return <>{fallback}</>;
  const Piece = entry.component as unknown as ComponentType<WidgetProps<K>>;
  return (
    <Guarded loading={loading}>
      <Piece {...props} />
    </Guarded>
  );
}
