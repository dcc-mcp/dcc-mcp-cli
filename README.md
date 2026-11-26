# DCCMCP CLI

`dccmcp` is the standalone command-line control plane for DCC-MCP. It
discovers DCC installations, connects agents to MCP endpoints, manages gateway
profiles, installs adapter skills, and exposes the generic MCP client surface.

The CLI source and release lifecycle live in this repository. Shared protocol,
gateway, transport, marketplace, and skill APIs remain in
[`dcc-mcp-core`](https://github.com/dcc-mcp/dcc-mcp-core) and are consumed as
versioned Git dependencies.

## Breaking change in 0.21.0

Version 0.21.0 moved the command-line control plane into this dedicated
repository. This is a breaking product boundary for installations that used
the CLI shipped from `dcc-mcp-core`.

- `dccmcp` is the primary executable name.
- `dcc-mcp-cli` remains as a compatibility binary for existing automation.
- New installations must use the installer and release assets from this
  repository.
- Core remains the source for shared runtime libraries, gateway/server
  contracts, and the `dcc-mcp-server` runtime.
- `dcc-cua` remains an independent runtime. The CLI forwards to it through
  `dccmcp cua --` and does not embed its SDK or host lifecycle.

Existing scripts can keep invoking `dcc-mcp-cli`. Update their installation
source to `https://github.com/dcc-mcp/dcc-mcp-cli` before removing the old Core
CLI package.

## Version 0.21.2

This release completes the post-boundary CLI maintenance flow.

- `dccmcp update` performs the normal CLI self-update. It checks the official
  release manifest, verifies SHA-256, and stages the next launch.
- `dccmcp --auto-update ...` enables the best-effort update before an ordinary
  command.
- `dccmcp cua -- ...` forwards arguments to the independently released
  `dcc-cua` runtime without embedding its SDK or host lifecycle.

The `dcc-mcp-cli` executable remains a compatibility alias. The DCC-CUA
runtime keeps its own version line and release process.

## Install

Linux and macOS

```bash
curl -fL https://raw.githubusercontent.com/dcc-mcp/dcc-mcp-cli/main/scripts/install-cli.sh -o install-cli.sh
cat install-cli.sh
sh ./install-cli.sh
```

Windows PowerShell

```powershell
Invoke-WebRequest https://raw.githubusercontent.com/dcc-mcp/dcc-mcp-cli/main/scripts/install-cli.ps1 -OutFile .\install-cli.ps1
Get-Content -Raw .\install-cli.ps1
& .\install-cli.ps1
```

The installer places `dccmcp` on `PATH` and creates the `dcc-mcp-cli`
compatibility alias beside it. Set `DCC_MCP_VERSION` or pass `--version` to
install a specific release.

Windows users can install the published package with
`winget install --id DccMcp.DccMcpCli --exact` when WinGet has accepted the
package. macOS users can install the maintained formula with
`brew tap dcc-mcp/tap && brew install dccmcp`.

## Update

The normal self-update flow is:

```bash
dccmcp update
```

This checks the release manifest, downloads the platform asset, verifies its
SHA-256 digest, and stages the binary for the next launch. Use the explicit
forms when a read-only or operator-controlled step is required:

```bash
dccmcp update check
dccmcp update apply

# Best-effort self-update before an ordinary command.
dccmcp --auto-update list
```

Set `DCC_MCP_CLI_AUTO_UPDATE=true` to enable the same best-effort behavior for
ordinary commands. Existing staged updates are reused. Automatic update
failures are reported as warnings and do not block the current command. The
release workflow publishes platform assets, ZIP archives, SHA-256 sidecars,
and update manifests for each release.

Package-manager upgrades use `winget upgrade --id DccMcp.DccMcpCli --exact` or
`brew upgrade dccmcp`.

## DCC UI Control and DCC-CUA

`dccmcp` is the control plane for the stable `ui-control` route. The project
owned DCC-CUA host performs exact-window binding, screenshots, UI Automation,
native input, visible safety effects, user interruption, and audit logging.
MCP-native clients may continue to use the compatibility `app_ui__*` tool
names. Before the first observation or input action, the host must report
`provider=dcc-cua`, its runtime version, and the exact target PID and HWND.
The same route and binding contract applies to an explicit DCC-CUA request for
a browser or another non-DCC application.

Use `dccmcp cua -- manifest` or `dccmcp cua -- ping` when the standalone CUA
runtime is installed. If it is missing, install the official `dcc-cua` release
separately or set `DCC_MCP_CUA_BINARY` to its absolute path. The CLI does not
download or embed the CUA runtime as part of its self-update.

The CLI does not embed the CUA SDK. Use the thin forwarding command when a
shell workflow needs the standalone runtime:

```bash
dccmcp cua -- manifest
dccmcp cua -- host-ensure
```

The `dcc-cua` runtime owns its long-lived Host process, permissions, sessions,
and UI safety boundary. It is released and updated independently. The CLI
resolves it from `DCC_MCP_CUA_BINARY`, beside the current CLI, the standard
installation directories, versioned installs, and then `PATH`. The structured
`ui-control` command remains the DCC-MCP route for scoped DCC calls.

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

## Package-manager release automation

Each tagged release publishes the standalone CLI assets first, then updates
the package managers from those immutable assets. The CLI repository requires
the following repository secrets for the two cross-repository jobs:

- `WINGET_TOKEN` with permission to read `microsoft/winget-pkgs`, push the
  `loonghao/winget-pkgs` fork, and open the update pull request.
- `HOMEBREW_TAP_TOKEN` with permission to push `dcc-mcp/homebrew-tap`.

Run `winget-first-manifest.yml` once for the initial `DccMcp.DccMcpCli`
submission. Later tags use the `publish-winget` job in `release.yml`. Every
later tag also regenerates `Formula/dccmcp.rb` in the Homebrew tap from the
published macOS universal2 binary. Missing package-manager credentials skip
the corresponding job with a warning and do not invalidate the CLI release.

## Repository boundary

This repository owns the CLI application, command UX, release artifacts,
installer scripts, and CLI-specific tests. Core owns the reusable MCP client
and DCC runtime contracts. The CLI consumes the published `dcc-mcp-core`
contracts through versioned Git dependencies. `dcc-cua` remains an independent
runtime and release boundary.
