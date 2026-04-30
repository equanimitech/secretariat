# Lexicons

AT-proto-style schema definitions for Secretariat record types. These are the
canonical wire shapes — the Rust types in `crates/core/src/domain/` mirror them
1:1 (field names, optional-ness, nested types).

## Status

**Unpublished.** These schemas are intentionally mutable until self-use
validates the primitive. We will only register them under the `tech.equanimi`
namespace once the shapes have stabilized through real correspondence.

Until then, treat this directory as **internal documentation of the on-wire
format**, not a public protocol.

## Files

| File | Lexicon ID | Used for |
|---|---|---|
| `tech.equanimi.secretariat.stamp.json` | `tech.equanimi.secretariat.stamp` | The signed human act block (`$attestation`) |
| `tech.equanimi.secretariat.envelope.json` | `tech.equanimi.secretariat.envelope` | The bid for attention block (`$envelope`) |
| `tech.equanimi.secretariat.attentionEnvelope.json` | `tech.equanimi.secretariat.attentionEnvelope` | The principal's published bounds |

## Why this exists alongside Rust types

The Rust newtypes are **authoritative for runtime validation** — they fail to
construct on bad input, which is the architectural guardrail. The lexicons are
**authoritative for the on-wire shape** — when v2 ships and Secretariat data
moves into a real PDS, these are the schemas the network consumes.

Keeping both in sync is a manual discipline today. When the schemas freeze
(post-v2), we can flip on lexicon-driven generation.

## Migration plan

1. **Day 1 (now):** lexicons live here as documentation, drive nothing at
   runtime.
2. **v2 (multi-correspondent):** lexicons published under `tech.equanimi`
   namespace. PDS ingests records validated against them.
3. **v3 (federation):** lexicons referenced from external clients; backwards
   compatibility becomes a constraint.
