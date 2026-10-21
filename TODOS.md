# TODOS

## Product

### Russian version of the client page

**What:** Add RU strings and the RU/EN switch to the client page.

**Why:** The original requirements ask for RU/EN; the CEO chose English only for the first release (2026-10-19).

**Context:** Keep every UI string in one dictionary from the first release so RU is a translation file plus the switch.

**Effort:** S
**Priority:** P2
**Depends on:** First release of KOL Arena

### Telegram alert on a failed daily collection

**What:** Notify the team in Telegram when a source run fails or a KOL goes stale.

**Why:** Today a failure is visible only on the admin "source health" page.

**Context:** `source_runs` already records status and error text; the alert reads it after each run. Needs a bot token and a chat id from the team.

**Effort:** S
**Priority:** P3
**Depends on:** First release; a Telegram bot from the team

### Shorter leaderboard on the home page

**What:** Show the first rows of the leaderboard and a "show more" control instead of all 135.

**Why:** The home page is about 59,000 px tall on a phone; the sections below the board are hard to reach.

**Context:** Found in the design audit 2026-10-21. Needs the CEO's choice of the first page size.

**Effort:** S
**Priority:** P2
**Depends on:** CEO decision

### Small design fixes from the design audit

**What:** Collection source line reads run together; fight "Who wins?" buttons touch; "Pick X to win" label uses the KOL id, not the name; KOL names with glyphs the fonts lack show empty boxes.

**Why:** Polish items left after the design audit 2026-10-21.

**Context:** `plugins/schedule/frontend/src/Collection.tsx`, the fight pick buttons in `plugins/fight/frontend/src/`.

**Effort:** S
**Priority:** P3
**Depends on:** None

### Small fixes from QA

**What:** Low items from QA 2026-10-21. Team picker: "Score from" above "Score to" shows "0 of 70" with no hint. Phone leaderboard card: the @handle sits on its own indented line. Changing the snapshot date clears the search.

**Why:** Low severity; nothing breaks.

**Context:** `plugins/picker/frontend/src/Picker.tsx`, `plugins/board/frontend/src/Leaderboard.tsx`, `plugins/board/frontend/src/Home.tsx`.

**Effort:** S
**Priority:** P3
**Depends on:** None

## Infrastructure

### Production deploy

**What:** Put KOL Arena on production.

**Why:** The first release goes to the shared test site only.

**Context:** Production is a separate decision of the owner (board rule). Secrets: Solana Tracker key, sheet URL, admin accounts.

**Effort:** M
**Priority:** P2
**Depends on:** Owner's decision

## Completed
