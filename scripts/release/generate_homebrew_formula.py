#!/usr/bin/env python3
"""Render the Homebrew formula for the standalone DCCMCP CLI."""

from __future__ import annotations

import argparse
from pathlib import Path
import re


_VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$")
_SHA256_RE = re.compile(r"^[0-9a-fA-F]{64}$")


def _validate(value: str, pattern: re.Pattern[str], label: str) -> str:
    if not pattern.fullmatch(value):
        raise ValueError(f"{label} is invalid: {value!r}")
    return value


def render_formula(*, version: str, url: str, sha256: str) -> str:
    """Return a deterministic formula for the macOS universal2 binary."""
    _validate(version, _VERSION_RE, "version")
    _validate(sha256, _SHA256_RE, "sha256")
    if not url.startswith("https://github.com/dcc-mcp/dcc-mcp-cli/releases/download/"):
        raise ValueError("url must point to an official dcc-mcp-cli release")
    if not url.endswith("/dccmcp-macos-universal2"):
        raise ValueError("url must point to the macOS universal2 dccmcp asset")

    return f'''class Dccmcp < Formula
  desc "DCCMCP command-line control plane for DCC discovery and MCP calls"
  homepage "https://github.com/dcc-mcp/dcc-mcp-cli"
  url "{url}"
  sha256 "{sha256.lower()}"
  version "{version}"
  license "MIT"

  def install
    bin.install "dccmcp-macos-universal2" => "dccmcp"
  end

  test do
    assert_match version.to_s, shell_output("#{{bin}}/dccmcp --version")
  end
end
'''


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--url", required=True)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    formula = render_formula(version=args.version, url=args.url, sha256=args.sha256)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(formula, encoding="utf-8")
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
