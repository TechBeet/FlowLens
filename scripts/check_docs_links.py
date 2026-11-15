#!/usr/bin/env python3
"""Fail if any relative link in a Markdown file under docs/ points at a missing path.

Checks ``[text](target)`` and ``![alt](target)`` style inline links plus
``[ref]: target`` reference definitions. Absolute URLs (http/https/mailto),
protocol-relative URLs and pure in-file anchors are skipped. Link targets that
carry an anchor/query suffix (``file.md#section``) are validated against the
path component only.

Exit code is 0 when every relative link resolves, 1 otherwise. No third-party
dependencies, so it runs on a bare ubuntu-latest runner.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

DOCS_ROOT_NAME = "docs"

# [label](target "optional title")  -- capture the target only.
INLINE_LINK = re.compile(r"!?\[[^\]]*\]\(\s*<?([^)\s>]+)>?(?:\s+[\"'][^\"']*[\"'])?\s*\)")
# [label]: target  -- reference-style definition.
REF_LINK = re.compile(r"^\s{0,3}\[[^\]]+\]:\s*<?([^\s>]+)>?", re.MULTILINE)

SKIP_PREFIXES = ("http://", "https://", "mailto:", "tel:", "//", "#", "data:")


def iter_targets(text: str) -> list[str]:
    """Return every link target found in a Markdown document."""
    return INLINE_LINK.findall(text) + REF_LINK.findall(text)


def check_file(md_file: Path, repo_root: Path) -> list[str]:
    """Return a list of human-readable problems for one Markdown file."""
    problems: list[str] = []
    try:
        text = md_file.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        problems.append(f"{md_file}: not valid UTF-8")
        return problems

    for raw in iter_targets(text):
        target = raw.strip()
        if not target or target.lower().startswith(SKIP_PREFIXES):
            continue

        # Drop ?query and #anchor before filesystem resolution.
        path_part = re.split(r"[?#]", target, maxsplit=1)[0]
        if not path_part:
            continue

        resolved = (md_file.parent / path_part).resolve()
        if not resolved.exists():
            rel_target = resolved.relative_to(repo_root) if resolved.is_relative_to(
                repo_root
            ) else resolved
            problems.append(
                f"{md_file.relative_to(repo_root)}: broken link -> {target} "
                f"(resolved: {rel_target})"
            )
    return problems


def main(argv: list[str]) -> int:
    repo_root = Path(argv[1]).resolve() if len(argv) > 1 else Path.cwd().resolve()
    docs_root = repo_root / DOCS_ROOT_NAME

    if not docs_root.is_dir():
        print(f"docs-links: '{DOCS_ROOT_NAME}/' not found under {repo_root}", file=sys.stderr)
        return 1

    md_files = sorted(docs_root.rglob("*.md"))
    if not md_files:
        print(f"docs-links: no Markdown files under {docs_root}", file=sys.stderr)
        return 1

    problems: list[str] = []
    link_count = 0
    for md_file in md_files:
        try:
            link_count += len(iter_targets(md_file.read_text(encoding="utf-8")))
        except UnicodeDecodeError:
            pass
        problems.extend(check_file(md_file, repo_root))

    print(f"docs-links: scanned {len(md_files)} file(s), {link_count} link(s)")

    if problems:
        print(f"docs-links: FAILED ({len(problems)} broken link(s))", file=sys.stderr)
        for problem in problems:
            print(f"  {problem}", file=sys.stderr)
        return 1

    print("docs-links: OK - every relative link under docs/ resolves")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
