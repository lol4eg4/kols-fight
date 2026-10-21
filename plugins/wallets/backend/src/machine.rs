//! The candidate state machine: which moves exist, and how candidates add up to a KOL status.

use api_wallets::{CandidateSource, CandidateStatus, KolWalletStatus};

use CandidateStatus::*;

/// Moves a candidate may make. Everything else is refused.
///
/// ```text
/// hypothesis -> confirmed | rejected | conflict
/// not_found  -> confirmed | conflict | retired
/// confirmed  -> revoked | conflict | retired
/// conflict   -> confirmed | rejected | revoked | retired
/// retired    -> confirmed | not_found | conflict   (the sheet shows the address again)
/// rejected, revoked: final (staff decided)
/// ```
pub fn allowed(from: CandidateStatus, to: CandidateStatus) -> bool {
    matches!(
        (from, to),
        (Hypothesis, Confirmed | Rejected | Conflict)
            | (NotFound, Confirmed | Conflict | Retired)
            | (Confirmed, Revoked | Conflict | Retired)
            | (Conflict, Confirmed | Rejected | Revoked | Retired)
            | (Retired, Confirmed | NotFound | Conflict)
    )
}

/// Where a candidate with this status counts for the KOL.
pub fn kol_status(statuses: impl IntoIterator<Item = CandidateStatus>) -> KolWalletStatus {
    let mut out = KolWalletStatus::NotFound;
    for s in statuses {
        let level = match s {
            Conflict => KolWalletStatus::Conflict,
            Confirmed => KolWalletStatus::Confirmed,
            Hypothesis => KolWalletStatus::Hypothesis,
            NotFound | Rejected | Revoked | Retired => KolWalletStatus::NotFound,
        };
        if rank(level) > rank(out) {
            out = level;
        }
    }
    out
}

fn rank(s: KolWalletStatus) -> u8 {
    match s {
        KolWalletStatus::NotFound => 0,
        KolWalletStatus::Hypothesis => 1,
        KolWalletStatus::Confirmed => 2,
        KolWalletStatus::Conflict => 3,
    }
}

pub fn status_str(s: CandidateStatus) -> &'static str {
    match s {
        Hypothesis => "hypothesis",
        Confirmed => "confirmed",
        NotFound => "not_found",
        Rejected => "rejected",
        Revoked => "revoked",
        Retired => "retired",
        Conflict => "conflict",
    }
}

pub fn parse_status(s: &str) -> Option<CandidateStatus> {
    Some(match s {
        "hypothesis" => Hypothesis,
        "confirmed" => Confirmed,
        "not_found" => NotFound,
        "rejected" => Rejected,
        "revoked" => Revoked,
        "retired" => Retired,
        "conflict" => Conflict,
        _ => return None,
    })
}

pub fn source_str(s: CandidateSource) -> &'static str {
    match s {
        CandidateSource::Kolexplorer => "kolexplorer",
        CandidateSource::Solanatracker => "solanatracker",
        CandidateSource::Fomo => "fomo",
        CandidateSource::FromKol => "from_kol",
        CandidateSource::Sheet => "sheet",
        CandidateSource::Pumpfun => "pumpfun",
        CandidateSource::Prototype => "prototype",
    }
}

pub fn parse_source(s: &str) -> Option<CandidateSource> {
    Some(match s {
        "kolexplorer" => CandidateSource::Kolexplorer,
        "solanatracker" => CandidateSource::Solanatracker,
        "fomo" => CandidateSource::Fomo,
        "from_kol" => CandidateSource::FromKol,
        "sheet" => CandidateSource::Sheet,
        "pumpfun" => CandidateSource::Pumpfun,
        "prototype" => CandidateSource::Prototype,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [CandidateStatus; 7] = [
        Hypothesis, Confirmed, NotFound, Rejected, Revoked, Retired, Conflict,
    ];

    #[test]
    fn final_states_do_not_move_and_nothing_moves_to_itself() {
        for to in ALL {
            assert!(!allowed(Rejected, to));
            assert!(!allowed(Revoked, to));
        }
        for s in ALL {
            assert!(!allowed(s, s));
        }
    }

    #[test]
    fn a_hypothesis_is_never_published_by_a_shortcut() {
        // The only way out of a hypothesis to a published state is `confirmed`.
        assert!(allowed(Hypothesis, Confirmed));
        assert!(!allowed(Hypothesis, Revoked));
        assert!(!allowed(Hypothesis, Retired));
        assert!(!allowed(Hypothesis, NotFound));
        assert!(!allowed(NotFound, Hypothesis));
        assert!(!allowed(Confirmed, Hypothesis));
    }

    #[test]
    fn precedence_of_candidates() {
        assert_eq!(kol_status([]), KolWalletStatus::NotFound);
        assert_eq!(
            kol_status([Rejected, Revoked, Retired, NotFound]),
            KolWalletStatus::NotFound
        );
        assert_eq!(
            kol_status([NotFound, Hypothesis]),
            KolWalletStatus::Hypothesis
        );
        assert_eq!(
            kol_status([Hypothesis, Confirmed]),
            KolWalletStatus::Confirmed
        );
        assert_eq!(
            kol_status([Confirmed, Conflict, Hypothesis]),
            KolWalletStatus::Conflict
        );
    }

    #[test]
    fn names_round_trip() {
        for s in ALL {
            assert_eq!(parse_status(status_str(s)), Some(s));
        }
        for s in [
            CandidateSource::Kolexplorer,
            CandidateSource::Solanatracker,
            CandidateSource::Fomo,
            CandidateSource::FromKol,
            CandidateSource::Sheet,
            CandidateSource::Pumpfun,
            CandidateSource::Prototype,
        ] {
            assert_eq!(parse_source(source_str(s)), Some(s));
        }
        assert_eq!(parse_status("nonsense"), None);
    }
}
