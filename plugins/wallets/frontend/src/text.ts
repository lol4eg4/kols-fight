import type { BadgeTone } from "@arena/design";
import { t } from "@arena/shell";
import type { CandidateSource, CandidateStatus, KolStatus } from "./overview";

// t() keys must be plain literals, so each lookup is a switch.

export function kolStatusText(s: KolStatus): string {
  switch (s) {
    case "confirmed":
      return t("wallets.kol_status.confirmed");
    case "hypothesis":
      return t("wallets.kol_status.hypothesis");
    case "not_found":
      return t("wallets.kol_status.not_found");
    case "conflict":
      return t("wallets.kol_status.conflict");
  }
}

export function kolStatusHint(s: KolStatus): string {
  switch (s) {
    case "confirmed":
      return t("wallets.kol_status.confirmed_hint");
    case "hypothesis":
      return t("wallets.kol_status.hypothesis_hint");
    case "not_found":
      return t("wallets.kol_status.not_found_hint");
    case "conflict":
      return t("wallets.kol_status.conflict_hint");
  }
}

export const KOL_TONES: Record<KolStatus, BadgeTone> = { confirmed: "plus", hypothesis: "coin", not_found: "muted", conflict: "minus" };
export const KOL_ICONS: Record<KolStatus, string> = { confirmed: "✓", hypothesis: "?", not_found: "–", conflict: "!" };

export function candidateStatusText(s: CandidateStatus): string {
  switch (s) {
    case "hypothesis":
      return t("wallets.status.hypothesis");
    case "confirmed":
      return t("wallets.status.confirmed");
    case "not_found":
      return t("wallets.status.not_found");
    case "rejected":
      return t("wallets.status.rejected");
    case "revoked":
      return t("wallets.status.revoked");
    case "retired":
      return t("wallets.status.retired");
    case "conflict":
      return t("wallets.status.conflict");
  }
}

export const CANDIDATE_TONES: Record<CandidateStatus, BadgeTone> = {
  hypothesis: "coin",
  confirmed: "plus",
  not_found: "muted",
  rejected: "muted",
  revoked: "minus",
  retired: "muted",
  conflict: "minus",
};

export function sourceText(s: CandidateSource): string {
  switch (s) {
    case "kolexplorer":
      return t("wallets.source.kolexplorer");
    case "solanatracker":
      return t("wallets.source.solanatracker");
    case "fomo":
      return t("wallets.source.fomo");
    case "from_kol":
      return t("wallets.source.from_kol");
    case "sheet":
      return t("wallets.source.sheet");
    case "pumpfun":
      return t("wallets.source.pumpfun");
    case "prototype":
      return t("wallets.source.prototype");
  }
}
