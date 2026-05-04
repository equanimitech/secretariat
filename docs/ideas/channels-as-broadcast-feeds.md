# Channels as broadcast feeds (RSS-shaped, public or private)

Raw capture — 2026-05-05.

- "Do we need channels? Do they simply look like broadcast messages (like RSS feeds) that are public or private?"
- Today's primitive is bilateral correspondence — one inviter, one claimant, one shared contract, two endpoints exchanging stamped envelopes. A channel is the same primitive in 1-to-many shape.
- Mental model: a **channel = a feed of stamped envelopes published by one DID, addressed to many DIDs (or to the public)**. Every entry is still a stamped envelope; there's no new wire format. The only new thing is *delivery shape* (broadcast vs unicast).
- Two channel modes:
  - **Public** — anyone can subscribe; envelope body unencrypted (or encrypted with a publicly-derived shared key). Reader fetches from a relay that hosts the feed. RSS/Atom-shaped.
  - **Private** — explicit subscriber list (DIDs); envelope body sealed to each subscriber's key (multi-recipient sealed-box, or per-subscriber re-encryption). Reader is the same client, just gated.
- Why this is interesting:
  - **No new ceremony.** Stamp is still the human-attested act. A channel post is just an envelope where `to:` is a list (or a "public" sentinel) instead of a single DID. Same Touch ID, same provenance.
  - **The feed itself becomes a stamped artifact.** A channel's metadata (title, owner DID, subscriber list, posting policy) is itself a signed object. Subscribers can verify "this feed is run by Rafa" cryptographically.
  - **Replaces 90% of "Substack"-shaped tools** for professional communications. Authors publishing chapters; advisors publishing market notes; lawyers publishing client briefs. Each post stamped, each reader verifying.
  - **Replaces Slack channels** for tighter teams. Same bilateral primitive, just N-bilateral. Strategic friction (each post requires a stamp) reduces the "hot take" volume — channels self-curate.
- Adjacent ideas:
  - The bubble-up idea (`docs/ideas/bubble-up-like-hey.md`) applies to channel posts too — defer a feed entry to surface again at review time.
  - The multi-granularity envelopes idea (`docs/ideas/multi-granularity-envelopes.md`) maps onto channels nicely — feed entries naturally come at multiple scales (headline-feed for skimming, full-text-feed for reading).
  - The scribe-background-journal idea (`docs/ideas/scribe-background-journaling.md`) — a private channel "to myself" *is* a journal. Same primitive.
- Questions:
  - **Wire-level**: does `$envelope.to` become `to: <did> | [<did>...] | "public"`, or is "channel" a separate `$type`? Cleaner to overload `to` — channels are an addressing pattern, not a new envelope class.
  - **Subscription mechanism**: how does a reader subscribe? Probably another invite primitive — a "subscribe to this channel" invite that adds the channel feed URL + its owner DID to the reader's contact-shaped list of subscribed feeds.
  - **Discovery**: how do readers find public channels? Same answer as DIDs — the relay can host a directory if it chooses, or the channel owner publishes a `did:web` resource pointing at it. Don't centralize.
  - **Posting cadence**: does the author have to manually stamp each post? Yes — that's the whole point. The bottleneck *is* the feature.
  - **Distinction from threads**: a thread is a subset of a channel where everyone can post. A channel restricts who posts. Maybe `permitted_posters: [<dids>]` is a third axis ("channel" = posters ⊆ subscribers; "thread" = posters = subscribers).
- Don't shape yet.
