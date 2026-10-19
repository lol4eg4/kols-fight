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

## Infrastructure

### Production deploy

**What:** Put KOL Arena on production.

**Why:** The first release goes to the shared test site only.

**Context:** Production is a separate decision of the owner (board rule). Secrets: Solana Tracker key, sheet URL, admin accounts.

**Effort:** M
**Priority:** P2
**Depends on:** Owner's decision

## Completed
