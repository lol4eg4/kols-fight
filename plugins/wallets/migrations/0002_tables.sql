-- Wallet candidates of network KOLs. A hypothesis address lives here and nowhere public.
CREATE TABLE wallets.candidate (
    id            bigserial PRIMARY KEY,
    kol           text NOT NULL,
    address       text NOT NULL,
    -- hypothesis, confirmed, not_found, rejected, revoked, retired, conflict
    status        text NOT NULL,
    -- kolexplorer, solanatracker, fomo, from_kol, sheet, pumpfun, prototype
    source        text NOT NULL,
    evidence      text,
    sent_by       text,
    confirmation  text,
    reason        text,
    main          boolean NOT NULL DEFAULT false,
    -- Last trade seen at the last trades check.
    last_trade_at timestamptz,
    checked_at    timestamptz,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL DEFAULT now(),
    UNIQUE (kol, address)
);

CREATE INDEX candidate_kol ON wallets.candidate (kol);
