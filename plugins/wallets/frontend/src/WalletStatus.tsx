import { Badge, NoData } from "@arena/design";
import { t } from "@arena/shell";
import { useOverview } from "./useOverview";
import { KOL_ICONS, KOL_TONES, kolStatusHint, kolStatusText } from "./text";

/** Filler of roster/wallet_status: the wallet status of one network KOL, from the shared overview. */
export default function WalletStatus({ kol }: { kol: string }) {
  const { load } = useOverview();
  if (load.status === "loading") return null;
  if (load.status === "error") return <NoData reason={t("wallets.filler.unavailable", { message: load.error.message })} />;
  const k = load.data.kols.find((x) => x.kol === kol);
  if (!k) return <NoData reason={t("wallets.filler.unknown")} />;
  return (
    <Badge tone={KOL_TONES[k.status]} icon={KOL_ICONS[k.status]} hint={kolStatusHint(k.status)}>
      {t("wallets.filler.label", { status: kolStatusText(k.status) })}
    </Badge>
  );
}
