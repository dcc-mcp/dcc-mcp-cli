# DCCMCP CLI release notes

## 0.23.0

- Migrate install receipts, rollback, uninstall, verification, components, host discovery, feedback and record/replay from Core.
- Retain both executable names, direct MCP commands and forwarding to the independently installed CUA runtime.
- CLI updates use the standalone release manifest. Server updates use the gateway. Read-only commands preserve staged updates.
- Pin shared runtime crates to Core 0.22.1. CLI and Core versions advance independently.

## 0.21.0

This release moves the command-line control plane into its own repository and
makes `dccmcp` the primary command. Existing `dcc-mcp-cli` scripts remain
supported as a compatibility path.
