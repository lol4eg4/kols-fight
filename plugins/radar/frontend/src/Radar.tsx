import { Card, EmptyState, ErrorLine, Heading, List, ListItem, PixelAvatar, Skeleton } from "@arena/design";
import { t, useApi } from "@arena/shell";

interface RadarKol {
  id: string;
  name: string;
  x_handle: string | null;
  wallet: string;
}

interface RadarView {
  date: string;
  status: "ok" | "not_updated";
  list_date: string | null;
  kols: RadarKol[];
}

/** Fills board/radar_section: today's Solana Tracker KOLs that are not on the board. */
export default function Radar({ date }: { date: string }) {
  const state = useApi<RadarView>(`/api/radar?date=${encodeURIComponent(date)}`);

  let body;
  if (state.status === "loading") {
    body = <Skeleton label={t("radar.loading")} variant="rows" count={3} />;
  } else if (state.status === "error") {
    body = <ErrorLine message={t("radar.error.load", { message: state.error.message })} onRetry={state.reload} retryLabel={t("radar.retry")} />;
  } else if (state.data.status === "not_updated") {
    const last = state.data.list_date;
    body = <EmptyState message={last ? t("radar.not_updated", { date: last }) : t("radar.not_updated_never")} />;
  } else if (state.data.kols.length === 0) {
    body = <EmptyState message={t("radar.empty")} />;
  } else {
    body = (
      <List label={t("radar.list")} variant="cards">
        {state.data.kols.map((k) => (
          <ListItem key={k.id}>
            <PixelAvatar id={k.id} size={32} />
            <span>{k.name}</span>{" "}
            <a href={`https://solscan.io/account/${k.wallet}`} target="_blank" rel="noopener noreferrer" aria-label={t("radar.solscan_for", { name: k.name })}>
              {t("radar.solscan")}
            </a>
          </ListItem>
        ))}
      </List>
    );
  }

  return (
    <Card as="section" aria-label={t("radar.title")}>
      <Heading level={2} variant="title">
        {t("radar.title")}
      </Heading>
      {body}
    </Card>
  );
}
