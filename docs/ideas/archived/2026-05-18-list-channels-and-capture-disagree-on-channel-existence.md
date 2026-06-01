---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T084231Z-gvrniw.md
---
# `list_channels` and `capture` disagree on channel existence

- `list_channels` returns any dir containing envelopes (showed `inbox:pain` with 2 envelopes).
- `capture` rejects same handle as "does not exist" because there's no `channel.md` manifest.
- Two source-of-truth definitions in same MCP server. List = "has envelopes," capture = "has manifest."
- Hit today: tried to capture into `inbox:pain` listed in `list_channels` output — got "channel does not exist." Had to call `create_channel` on already-existing dir to backfill `channel.md`. Worked but confusing.
- Fix shape: either `list_channels` filters out dirs without manifest, or `capture` accepts dirs as channels by virtue of envelopes existing. The legacy pre-manifest dirs need a migration pass either way.

- Don't fix yet.
