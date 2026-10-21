import { Badge, Card, EmptyState, ErrorLine, Heading, List, ListItem, Money, NoData, PixelAvatar, ScoreBar, Skeleton, TierBadge } from "@arena/design";
import { Widget, t } from "@arena/shell";
import { useNetwork } from "./useNetwork";
import { waitText } from "./view";
import type { OnBoard, Waiting } from "./view";
import "./network.css";

function reasonText(w: Waiting): string {
  switch (waitText(w)) {
    case "not_confirmed":
      return t("network.wait.not_confirmed");
    case "quiet_one_day":
      return t("network.wait.quiet_one_day");
    case "quiet_days":
      return t("network.wait.quiet_days", { days: w.quiet_days ?? 0 });
    default:
      return t("network.wait.quiet");
  }
}

function MemberRow({ m }: { m: OnBoard }) {
  return (
    <ListItem>
      <div className="net-row">
        <div className="net-row__who">
          <PixelAvatar id={m.kol} size={32} />
          <strong className="net-row__name">{m.name}</strong>
          {m.tier && <TierBadge tier={m.tier} />}
          {m.rank !== null && <span className="net-row__rank">{t("network.rank", { rank: m.rank })}</span>}
        </div>
        <dl className="net-row__stats">
          <div>
            <dt>{t("network.col.score")}</dt>
            <dd>{m.score !== null && m.tier ? <ScoreBar score={m.score} tier={m.tier} label={t("network.score_of", { name: m.name })} /> : <NoData reason={t("network.no_score")} />}</dd>
          </div>
          <div>
            <dt>{t("network.col.pnl")}</dt>
            <dd>
              <Money value={m.pnl_30d} reason={t("network.no_pnl")} />
            </dd>
          </div>
          <div>
            <dt>{t("network.col.volume")}</dt>
            <dd>
              <Money value={m.volume_30d} signed={false} reason={t("network.no_volume")} />
            </dd>
          </div>
          <div>
            <dt>{t("network.col.days")}</dt>
            <dd className="t-mono">{m.days_in_market !== null ? t("network.days", { days: m.days_in_market }) : <NoData reason={t("network.no_days")} />}</dd>
          </div>
        </dl>
        <div className="net-row__action">
          <Widget name="fight/add_to_fight" props={{ kol: m.kol }} />
        </div>
      </div>
    </ListItem>
  );
}

/** Fills board/network_section: «Our network», the network KOLs on the board and the waiting list. */
export default function NetworkSection({ date }: { date: string }) {
  const state = useNetwork(date);

  let body;
  if (state.status === "loading") {
    body = <Skeleton label={t("network.loading")} variant="rows" count={4} />;
  } else if (state.status === "error") {
    body =
      state.error.code === "no_snapshot" ? (
        <EmptyState message={t("network.no_board")} />
      ) : (
        <ErrorLine message={t("network.error.load", { message: state.error.message })} onRetry={state.reload} retryLabel={t("network.retry")} />
      );
  } else if (state.data.members.length === 0 && state.data.waiting.length === 0) {
    body = <EmptyState message={t("network.empty")} />;
  } else {
    const { members, waiting } = state.data;
    body = (
      <>
        {members.length === 0 ? (
          <EmptyState message={t("network.none_on_board")} />
        ) : (
          <List label={t("network.members")}>
            {members.map((m) => (
              <MemberRow key={m.kol} m={m} />
            ))}
          </List>
        )}
        {waiting.length > 0 && (
          <>
            <Heading level={3} variant="title">
              {t("network.waiting.title")}
            </Heading>
            <List label={t("network.waiting.title")}>
              {waiting.map((w) => (
                <ListItem key={w.kol}>
                  <div className="net-wait">
                    <PixelAvatar id={w.kol} size={24} />
                    <span>{w.name}</span>
                    <span className="net-wait__reason">{reasonText(w)}</span>
                  </div>
                </ListItem>
              ))}
            </List>
          </>
        )}
      </>
    );
  }

  return (
    <Card as="section" aria-label={t("network.title")}>
      <div className="net-head">
        <Heading level={2} variant="title">
          {t("network.title")}
        </Heading>
        <Badge tone="lavender" shape="pill" icon="🤝">
          {t("network.chip")}
        </Badge>
      </div>
      {body}
    </Card>
  );
}
