#!/usr/bin/env python3
"""Spike: carry Secretariat stamp state into a Hister index via the `label` field.

Hister's `metadata` map is mapped `noIdxMap` (server/indexer/indexer.go:2095), so it is
stored but never searchable. `label` IS indexed (:2087) and is a declared search field
(server/indexer/searchschema/schema.go), which makes it the only no-fork carrier for
per-document stamp state.

Emits Hister export-JSONL on stdout, one Document per line, for:

    hister import file <out.jsonl>

Hard rule #5 ("a document whose signature fails is malformed and must be quarantined,
not surfaced") is enforced here: tampered and unverifiable documents are NEVER emitted.
They are reported on stderr instead. A search index that ranks a tampered body among
ordinary hits is the exact failure that rule forbids.
"""

from __future__ import annotations

import json
import subprocess
import sys
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path

SEC = "/Applications/Secretariat.app/Contents/MacOS/sec"  # hard rule #8: prod binary
DOC_TYPE_LOCAL = 1  # document.Local — server/document/type.go

# Labels that may enter the index. Anything not here is quarantined.
LABEL_SEALED = "sealed"
LABEL_SIGNED = "signed"
LABEL_UNSIGNED = "unsigned"
INDEXABLE = {LABEL_SEALED, LABEL_SIGNED, LABEL_UNSIGNED}


@dataclass(frozen=True)
class Verdict:
    label: str
    signature: str
    stamp: str
    detail: str = ""

    @property
    def indexable(self) -> bool:
        return self.label in INDEXABLE


def verify(path: Path) -> Verdict:
    """Classify one document.

    `sec verify` exits 0 even when it fails to parse a file (observed on a legacy
    `app.equanimi.secretariat.stamp` $type), writing the error to stderr and nothing
    to stdout. Trusting returncode here would silently label a corrupt doc "unsigned",
    so the JSON parse is the real gate.
    """
    try:
        proc = subprocess.run(
            [SEC, "verify", "--json", str(path)],
            capture_output=True, text=True, timeout=60,
        )
    except subprocess.TimeoutExpired:
        return Verdict("unverifiable", "?", "?", "verify timed out")

    try:
        result = json.loads(proc.stdout)
    except json.JSONDecodeError:
        detail = (proc.stderr or "").strip().splitlines()
        return Verdict("unverifiable", "?", "?", detail[-1] if detail else "no JSON on stdout")

    signature = result.get("signature", {}).get("outcome", "?")
    stamp = result.get("stamp", {}).get("outcome", "?")

    if signature == "tampered":
        return Verdict("tampered", signature, stamp, "signature does not match body")
    if stamp == "verified":
        return Verdict(LABEL_SEALED, signature, stamp)
    if signature not in ("none", "?"):
        return Verdict(LABEL_SIGNED, signature, stamp)
    return Verdict(LABEL_UNSIGNED, signature, stamp)


def title_of(path: Path, body: str) -> str:
    for line in body.splitlines():
        line = line.strip()
        if line.startswith("# "):
            return line[2:].strip()
    return path.stem


def to_document(path: Path, verdict: Verdict) -> dict:
    body = path.read_text(encoding="utf-8", errors="replace")
    stat = path.stat()
    return {
        "url": f"file://{path.resolve()}",
        "domain": "",
        "title": title_of(path, body),
        "text": body,
        "type": DOC_TYPE_LOCAL,
        "label": verdict.label,
        "added": int(stat.st_mtime),
        "updated": int(stat.st_mtime),
        # Stored but NOT searchable — kept for the preview panel and to prove the
        # metadata/label split empirically.
        "metadata": {"signature": verdict.signature, "stamp": verdict.stamp},
    }


def main() -> int:
    if len(sys.argv) < 3:
        print(f"usage: {sys.argv[0]} <docs-dir> <out.jsonl>", file=sys.stderr)
        return 2

    root, out_path = Path(sys.argv[1]), Path(sys.argv[2])
    files = sorted(root.rglob("*.md"))
    if not files:
        print(f"no markdown under {root}", file=sys.stderr)
        return 1

    with ThreadPoolExecutor(8) as pool:
        verdicts = list(pool.map(verify, files))

    emitted, quarantined = 0, []
    with out_path.open("w", encoding="utf-8") as out:
        for path, verdict in zip(files, verdicts):
            if not verdict.indexable:
                quarantined.append((path, verdict))
                continue
            out.write(json.dumps(to_document(path, verdict), ensure_ascii=False) + "\n")
            emitted += 1

    tally = Counter(v.label for v in verdicts)
    print(f"scanned {len(files)} · emitted {emitted} · quarantined {len(quarantined)}", file=sys.stderr)
    for label, count in tally.most_common():
        mark = " " if label in INDEXABLE else "!"
        print(f"  {mark} {label:<14} {count}", file=sys.stderr)

    if quarantined:
        print("\nQUARANTINED (hard rule #5 — not surfaced):", file=sys.stderr)
        for path, verdict in quarantined:
            print(f"  {path}\n      {verdict.label}: {verdict.detail}", file=sys.stderr)

    return 0


if __name__ == "__main__":
    sys.exit(main())
