use super::{endpoint_for_mcp, parse_json_object, to_json};
use anyhow::Context;
use clap::Subcommand;
use dcc_mcp_client::McpHttpClient;
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Debug, Subcommand)]
pub(crate) enum McpAction {
    /// Perform the MCP initialize handshake and print the negotiated session.
    Initialize(McpInitializeArgs),
    /// Work with MCP tools.
    Tools {
        #[command(subcommand)]
        action: McpToolsAction,
    },
    /// Work with MCP resources.
    Resources {
        #[command(subcommand)]
        action: McpResourcesAction,
    },
    /// Work with MCP prompts.
    Prompts {
        #[command(subcommand)]
        action: McpPromptsAction,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum McpToolsAction {
    /// List all tools, following MCP pagination cursors.
    List(McpConnectionArgs),
    /// Call one MCP tool by its server-visible name.
    Call(McpToolCallArgs),
}

#[derive(Debug, Subcommand)]
pub(crate) enum McpResourcesAction {
    /// List all resources, following MCP pagination cursors.
    List(McpConnectionArgs),
    /// Read one resource by URI.
    Read(McpResourceReadArgs),
}

#[derive(Debug, Subcommand)]
pub(crate) enum McpPromptsAction {
    /// List all prompts, following MCP pagination cursors.
    List(McpConnectionArgs),
    /// Render one prompt with JSON arguments.
    Get(McpPromptGetArgs),
}

#[derive(Debug, Clone, clap::Args)]
pub(crate) struct McpConnectionArgs {
    /// MCP endpoint, for example http://127.0.0.1:9765/mcp.
    #[arg(long, env = "DCC_MCP_MCP_URL")]
    url: Option<String>,
    /// Timeout for each HTTP request.
    #[arg(long)]
    timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, clap::Args)]
pub(crate) struct McpInitializeArgs {
    #[command(flatten)]
    connection: McpConnectionArgs,
    /// Client name advertised in initialize.
    #[arg(long, default_value = "dccmcp")]
    client_name: String,
    /// Client version advertised in initialize.
    #[arg(long, default_value = env!("CARGO_PKG_VERSION"))]
    client_version: String,
}

#[derive(Debug, Clone, clap::Args)]
pub(crate) struct McpToolCallArgs {
    #[command(flatten)]
    connection: McpConnectionArgs,
    /// Server-visible MCP tool name.
    #[arg(long)]
    name: String,
    /// JSON object passed as MCP tool arguments.
    #[arg(long = "json", default_value = "{}")]
    arguments_json: String,
    /// Optional JSON object passed as MCP `_meta`.
    #[arg(long)]
    meta_json: Option<String>,
}

#[derive(Debug, Clone, clap::Args)]
pub(crate) struct McpResourceReadArgs {
    #[command(flatten)]
    connection: McpConnectionArgs,
    /// Resource URI.
    #[arg(long)]
    uri: String,
}

#[derive(Debug, Clone, clap::Args)]
pub(crate) struct McpPromptGetArgs {
    #[command(flatten)]
    connection: McpConnectionArgs,
    /// Prompt name.
    #[arg(long)]
    name: String,
    /// JSON object containing prompt argument values.
    #[arg(long = "arguments-json", default_value = "{}")]
    arguments_json: String,
}

pub(crate) async fn run(action: McpAction, default_base_url: &str) -> anyhow::Result<Value> {
    match action {
        McpAction::Initialize(args) => {
            let mut client = new_mcp_client(&args.connection, default_base_url)?
                .with_client_info(args.client_name, args.client_version);
            to_json(client.initialize().await?)
        }
        McpAction::Tools { action } => match action {
            McpToolsAction::List(args) => {
                let mut client = new_mcp_client(&args, default_base_url)?;
                let tools = client.tools_list_all().await?;
                Ok(json!({
                    "count": tools.len(),
                    "tools": tools,
                }))
            }
            McpToolsAction::Call(args) => {
                let arguments = parse_json_object(&args.arguments_json, "--json")?;
                let meta = args
                    .meta_json
                    .as_deref()
                    .map(|raw| parse_json_object(raw, "--meta"))
                    .transpose()?;
                let mut client = new_mcp_client(&args.connection, default_base_url)?;
                to_json(client.tools_call(args.name, arguments, meta).await?)
            }
        },
        McpAction::Resources { action } => match action {
            McpResourcesAction::List(args) => {
                let mut client = new_mcp_client(&args, default_base_url)?;
                let resources = client.resources_list_all().await?;
                Ok(json!({
                    "count": resources.len(),
                    "resources": resources,
                }))
            }
            McpResourcesAction::Read(args) => {
                let mut client = new_mcp_client(&args.connection, default_base_url)?;
                to_json(client.resources_read(args.uri).await?)
            }
        },
        McpAction::Prompts { action } => match action {
            McpPromptsAction::List(args) => {
                let mut client = new_mcp_client(&args, default_base_url)?;
                let prompts = client.prompts_list_all().await?;
                Ok(json!({
                    "count": prompts.len(),
                    "prompts": prompts,
                }))
            }
            McpPromptsAction::Get(args) => {
                let arguments = parse_json_object(&args.arguments_json, "--arguments-json")?
                    .as_object()
                    .cloned()
                    .context("--arguments-json must be a JSON object")?;
                let mut client = new_mcp_client(&args.connection, default_base_url)?;
                to_json(client.prompts_get(args.name, arguments).await?)
            }
        },
    }
}

fn new_mcp_client(
    connection: &McpConnectionArgs,
    default_base_url: &str,
) -> anyhow::Result<McpHttpClient> {
    let raw_url = connection.url.as_deref().unwrap_or(default_base_url);
    let mcp_url = endpoint_for_mcp(raw_url);
    Ok(McpHttpClient::with_timeout(
        mcp_url,
        Duration::from_secs(connection.timeout_secs.unwrap_or(30).max(1)),
    )?)
}

pub(crate) fn url(action: &McpAction) -> Option<&str> {
    match action {
        McpAction::Initialize(args) => args.connection.url.as_deref(),
        McpAction::Tools { action } => match action {
            McpToolsAction::List(args) => args.url.as_deref(),
            McpToolsAction::Call(args) => args.connection.url.as_deref(),
        },
        McpAction::Resources { action } => match action {
            McpResourcesAction::List(args) => args.url.as_deref(),
            McpResourcesAction::Read(args) => args.connection.url.as_deref(),
        },
        McpAction::Prompts { action } => match action {
            McpPromptsAction::List(args) => args.url.as_deref(),
            McpPromptsAction::Get(args) => args.connection.url.as_deref(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::cli::{Args, Command};
    use clap::Parser;

    #[test]
    fn dccmcp_mcp_tools_call_contract_parses_shared_client_arguments() {
        let args = Args::try_parse_from([
            "dccmcp",
            "mcp",
            "tools",
            "call",
            "--url",
            "http://127.0.0.1:9765/mcp",
            "--name",
            "create_sphere",
            "--json",
            r#"{"radius":2}"#,
            "--meta-json",
            r#"{"trace":"task-1"}"#,
        ])
        .expect("parse generic MCP tools call");

        let Command::Mcp {
            action:
                McpAction::Tools {
                    action: McpToolsAction::Call(call),
                },
        } = args.command
        else {
            panic!("expected MCP tools call");
        };

        assert_eq!(
            call.connection.url.as_deref(),
            Some("http://127.0.0.1:9765/mcp")
        );
        assert_eq!(call.name, "create_sphere");
        assert_eq!(call.arguments_json, r#"{"radius":2}"#);
    }

    #[test]
    fn dccmcp_mcp_resources_and_prompts_contracts_parse() {
        let resources = Args::try_parse_from([
            "dccmcp",
            "mcp",
            "resources",
            "read",
            "--url",
            "http://127.0.0.1:9765/mcp",
            "--uri",
            "scene://active",
        ])
        .expect("parse resource read");
        assert!(matches!(
            resources.command,
            Command::Mcp {
                action: McpAction::Resources {
                    action: McpResourcesAction::Read(_)
                }
            }
        ));

        let prompts = Args::try_parse_from([
            "dccmcp",
            "mcp",
            "prompts",
            "get",
            "--name",
            "render_review",
            "--arguments-json",
            r#"{"frame":"1001"}"#,
        ])
        .expect("parse prompt get");
        assert!(matches!(
            prompts.command,
            Command::Mcp {
                action: McpAction::Prompts {
                    action: McpPromptsAction::Get(_)
                }
            }
        ));
    }
}
