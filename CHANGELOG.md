# Changelog

## 2026-10-21 — First release of KOL Arena

Clients get a live ranking of Solana KOLs and a fight between any 2 to 4. The team gets a closed section to run the network.

### For clients

- You can now see Solana KOLs ranked by Arena Score from 0 to 100, with tiers GOAT, Diamond, Sendoor and Degen.
- You can now open any KOL's card: Score broken into four parts, badges, wallet and links.
- You can now pick 2 to 4 KOLs with «Fight» and watch the fight, then share its link or picture on X.
- You can now play the duel, guess the winner, follow streaks, moves, the radar and the tournament.
- The leaderboard shows 20 rows first; «Show more» adds 20 at a time.

### For the team

- You can now sign in to `/team`. Admins add and revoke staff.
- You can now confirm, reject, revoke or make main a KOL's wallet, and resolve an address claimed by two KOLs. Only confirmed wallets reach the client page.
- You can now find a wallet from 2 or 3 FOMO positions.
- You can now record a KOL's verified X handle, formats and reach.
- You can now read the network sheet and download the network CSV. Rates and notes stay in the team section.
- You can now pick KOLs for a client and export the list with or without rates.
- You can now watch the daily collection, run it for today, or re-rank a past date.

### Under the hood

- Data collects daily at 06:00 UTC from KOL Explorer and Solana Tracker; the network sheet is read at 05:45 UTC.
- Every block can be turned off with `ARENA_IMPL_RULES` without a rebuild.
- Docs: tutorial, how-to, reference and explanation in `docs/`.
