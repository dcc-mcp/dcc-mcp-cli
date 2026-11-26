# DCCMCP CLI

`dccmcp` is the standalone command-line control plane for DCC-MCP. It
discovers DCC installations, connects agents to MCP endpoints, manages gateway
profiles, installs adapter skills, and exposes the generic MCP client surface.

The CLI source and release lifecycle live in this repository. Shared protocol,
gateway, transport, marketplace, and skill APIs remain in
[`dcc-mcp-core`](https://github.com/dcc-mcp/dcc-mcp-core) and are consumed as
versioned Git dependencies.

## Version 0.21.0

Version 0.21.0 is the first standalone CLI release and marks a breaking
product boundary.

- `dccmcp` is the primary executable name.
- `dcc-mcp-cli` remains as a compatibility binary for existing automation.
- The reusable MCP Streamable HTTP client is supplied by `dcc-mcp-client` in
  dcc-mcp-core.
- New integrations should depend on this repository for the CLI and on core
  for shared runtime libraries.

## Install

Linux and macOS

```bash
curl -fsSL https://raw.githubusercontent.com/dcc-mcp/dcc-mcp-cli/main/scripts/install-cli.sh | sh
```

Windows PowerShell

```powershell
irm https://raw.githubusercontent.com/dcc-mcp/dcc-mcp-cli/main/scripts/install-cli.ps1 | iex
```

The installer places `dccmcp` on `PATH` and creates the `dcc-mcp-cli`
compatibility alias beside it. Set `DCC_MCP_VERSION` or pass `--version` to
install a specific release.

## Update

The supported update flow is:

```bash
dccmcp update check
dccmcp update apply
```

`update apply` stages the binary for the next launch. The release workflow
publishes platform assets, ZIP archives, SHA-256 sidecars, and update
manifests for each release.

## MCP calls

```bash
dccmcp mcp initialize --url http://127.0.0.1:9765/mcp
dccmcp mcp tools list --url http://127.0.0.1:9765/mcp
dccmcp mcp tools call --url http://127.0.0.1:9765/mcp \
  --name create_sphere --json '{"radius": 2.0}'
```

## Build

The Windows target is always explicit MSVC.

```powershell
$env:CARGO_TARGET_DIR = "target-msvc"
$env:RUSTUP_TOOLCHAIN = "1.95.0-x86_64-pc-windows-msvc"
rustup target add x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc -p dcc-mcp-cli --bin dccmcp --bin dcc-mcp-cli
cargo test -p dcc-mcp-cli --tests --target x86_64-pc-windows-msvc
```

Linux and macOS use their native targets. The release workflow builds the
Windows artifact with `x86_64-pc-windows-msvc` and does not use the GNU target.

## Repository boundary

This repository owns the CLI application, command UX, release artifacts,
installer scripts, and CLI-specific tests. Core owns the reusable MCP client
and DCC runtime contracts. The CLI pins the core dependency branch during the
0.21.0 transition and will move to the `v0.21.0` core tag when that release
lands.
