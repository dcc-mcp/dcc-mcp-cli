#!/usr/bin/env python3
"""Generate a platform-specific DCCMCP update manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def generate_manifest(
    *,
    version: str,
    platform: str,
    release_tag: str,
    repo: str,
    dccmcp_bin: Path,
    compatibility_bin: Path,
    out_dir: Path,
) -> Path:
    assets = {
        "dccmcp": dccmcp_bin,
        "dcc-mcp-cli": compatibility_bin,
    }
    manifest = {
        name: {
            "version": version,
            "url": f"https://github.com/{repo}/releases/download/{release_tag}/{path.name}",
            "sha256": sha256(path),
            "release_notes": f"https://github.com/{repo}/releases/tag/{release_tag}",
        }
        for name, path in assets.items()
        if path.is_file()
    }
    if len(manifest) != len(assets):
        missing = [name for name, path in assets.items() if not path.is_file()]
        raise FileNotFoundError(f"missing release assets: {', '.join(missing)}")

    out_dir.mkdir(parents=True, exist_ok=True)
    target = out_dir / f"dcc-mcp-update-manifest-{platform}.json"
    target.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return target


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--release-tag", required=True)
    parser.add_argument("--repo", default="dcc-mcp/dcc-mcp-cli")
    parser.add_argument("--dccmcp-bin", type=Path, required=True)
    parser.add_argument("--compatibility-bin", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, default=Path())
    args = parser.parse_args()
    path = generate_manifest(
        version=args.version,
        platform=args.platform,
        release_tag=args.release_tag,
        repo=args.repo,
        dccmcp_bin=args.dccmcp_bin,
        compatibility_bin=args.compatibility_bin,
        out_dir=args.out_dir,
    )
    print(path.as_posix())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
