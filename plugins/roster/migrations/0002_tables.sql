-- Network members. Public-safe columns only; rates, group and notes live in roster.terms.
CREATE TABLE roster.member (
    kol         text PRIMARY KEY,
    name        text NOT NULL,
    -- FOMO handle as written in the sheet; the sheet row is keyed by its lowercase form.
    fomo_handle text,
    fomo_key    text,
    -- X handle: from the prototype seed or entered by staff; verified only when staff says so.
    x_handle    text,
    x_verified  boolean NOT NULL DEFAULT false,
    pumpfun     text,
    formats     text[] NOT NULL DEFAULT '{}',
    -- Stays a member without a sheet row (owner's decision, 9 prototype KOLs).
    keep        boolean NOT NULL DEFAULT false,
    active      boolean NOT NULL DEFAULT true,
    joined_at   timestamptz NOT NULL DEFAULT now(),
    left_at     timestamptz,
    last_seen_at timestamptz
);

CREATE UNIQUE INDEX member_fomo_key ON roster.member (fomo_key) WHERE fomo_key IS NOT NULL;

-- Team-only data from the sheet: group, rates, notes, wallet cells. Never in a public DTO.
CREATE TABLE roster.terms (
    kol          text PRIMARY KEY REFERENCES roster.member (kol),
    grp          text,
    low_usd      double precision,
    high_usd     double precision,
    note         text,
    wallet_note  text,
    sheet_wallets text[] NOT NULL DEFAULT '{}',
    sheet_line   integer,
    updated_at   timestamptz NOT NULL DEFAULT now()
);

-- Membership history: nothing is deleted when a row leaves the sheet.
CREATE TABLE roster.membership_event (
    id     bigserial PRIMARY KEY,
    kol    text NOT NULL REFERENCES roster.member (kol),
    at     timestamptz NOT NULL DEFAULT now(),
    action text NOT NULL,
    reason text
);

CREATE INDEX membership_event_kol ON roster.membership_event (kol, at);

-- One row per sheet read, for the collection screen's «other sources» slot.
CREATE TABLE roster.sync_run (
    id          bigserial PRIMARY KEY,
    started_at  timestamptz NOT NULL,
    finished_at timestamptz NOT NULL,
    status      text NOT NULL,
    error       text,
    rows        integer NOT NULL DEFAULT 0,
    added       integer NOT NULL DEFAULT 0,
    updated     integer NOT NULL DEFAULT 0,
    removed     integer NOT NULL DEFAULT 0,
    skipped     jsonb NOT NULL DEFAULT '[]',
    note        text
);
