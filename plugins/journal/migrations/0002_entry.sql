-- Journal entries: who changed what, when, in which block. Written only through the API.
CREATE TABLE journal.entry (
    id        bigserial PRIMARY KEY,
    at        timestamptz NOT NULL DEFAULT now(),
    person_id bigint,
    role      text NOT NULL CHECK (role IN ('nobody', 'staff', 'admin')),
    block     text NOT NULL,
    object    text NOT NULL,
    action    text NOT NULL,
    before    jsonb,
    after     jsonb
);

CREATE INDEX entry_object ON journal.entry (block, object, id);
