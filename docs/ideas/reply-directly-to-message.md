# Reply directly to a message

Raw capture — 2026-05-04.

- "I should be able to reply directly to a message no?"
- Today's flow: read inbox envelope → switch to AI assistant → describe the reply → AI composes a new envelope → it lands in the review queue → stamp + send. Three context switches.
- "Reply" should be a first-class action in the review surface. Click an inbox envelope → see body → "Reply" button → composer opens (in-app or via the assistant) seeded with the right `to:`, the inbox envelope's headline as quoted context, maybe a thread reference.
- Adjacent: should envelopes carry an `in_reply_to` reference (envelope hash or message-id-style identifier)? That lights up threading later — view as conversation, not flat list.
- Questions:
  - Reply via in-app composer (depends on "native AI" idea above) or reply via the principal's AI assistant (sec-mcp)?
  - Should the protocol carry a thread/reply field now, or add later? Adding now is cheap; retrofitting later breaks compatibility.
  - Visual model: does the inbox surface threads (recipient/topic-based) or stays flat? Hey-style "bubble up" relevant.
- Don't shape yet.
