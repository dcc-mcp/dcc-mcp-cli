"""Tests for the generated Homebrew formula contract."""

from __future__ import annotations

import unittest

from generate_homebrew_formula import render_formula


VERSION = "0.21.1"
URL = f"https://github.com/dcc-mcp/dcc-mcp-cli/releases/download/v{VERSION}/dccmcp-macos-universal2"
SHA256 = "A" * 64


class HomebrewFormulaTests(unittest.TestCase):
    def test_formula_uses_the_official_universal2_asset(self) -> None:
        formula = render_formula(version=VERSION, url=URL, sha256=SHA256)

        self.assertIn("class Dccmcp < Formula", formula)
        self.assertIn(f'url "{URL}"', formula)
        self.assertIn(f'sha256 "{SHA256.lower()}"', formula)
        self.assertIn('bin.install "dccmcp-macos-universal2" => "dccmcp"', formula)
        self.assertIn('shell_output("#{bin}/dccmcp --version")', formula)

    def test_formula_rejects_non_official_assets(self) -> None:
        with self.assertRaisesRegex(ValueError, "official dcc-mcp-cli release"):
            render_formula(version=VERSION, url="https://example.invalid/dccmcp-macos-universal2", sha256=SHA256)

    def test_formula_rejects_invalid_digest(self) -> None:
        with self.assertRaisesRegex(ValueError, "sha256 is invalid"):
            render_formula(version=VERSION, url=URL, sha256="not-a-digest")


if __name__ == "__main__":
    unittest.main()
