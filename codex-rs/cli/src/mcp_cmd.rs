use std::collections::HashMap;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use clap::ArgGroup;
use codex_common::CliConfigOverrides;
use codex_common::format_env_display::format_env_display;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::find_codex_home;
use codex_core::config::load_global_mcp_servers;
use codex_core::config::write_global_mcp_servers;
use codex_core::config_types::McpServerConfig;
use codex_core::config_types::McpServerTransportConfig;

/// [experimental] Manage configured MCP servers.
///
/// Subcommands:
/// - `list`   — list configured servers (with `--json`)
/// - `get`    — show a single server (with `--json`)
/// - `add`    — add a server launcher entry to `~/.codex/config.toml`
/// - `remove` — delete a server entry
#[derive(Debug, clap::Parser)]
pub struct McpCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    pub subcommand: McpSubcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum McpSubcommand {
    /// [experimental] List configured MCP servers.
    List(ListArgs),

    /// [experimental] Show details for a configured MCP server.
    Get(GetArgs),

    /// [experimental] Add a global MCP server entry.
    Add(AddArgs),

    /// [experimental] Remove a global MCP server entry.
    Remove(RemoveArgs),
}

#[derive(Debug, clap::Parser)]
pub struct ListArgs {
    /// Output the configured servers as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, clap::Parser)]
pub struct GetArgs {
    /// Name of the MCP server to display.
    pub name: String,

    /// Output the server configuration as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, clap::Parser)]
pub struct AddArgs {
    /// Name for the MCP server configuration.
    pub name: String,

    #[command(flatten)]
    pub transport_args: AddMcpTransportArgs,
}

#[derive(Debug, clap::Args)]
#[command(
    group(
        ArgGroup::new("transport")
            .args(["command", "url"])
            .required(true)
            .multiple(false)
    )
)]
pub struct AddMcpTransportArgs {
    #[command(flatten)]
    pub stdio: Option<AddMcpStdioArgs>,

    #[command(flatten)]
    pub streamable_http: Option<AddMcpStreamableHttpArgs>,
}

#[derive(Debug, clap::Args)]
pub struct AddMcpStdioArgs {
    /// Command to launch the MCP server.
    /// Use --url for a streamable HTTP server.
    #[arg(trailing_var_arg = true, num_args = 0..)]
    pub command: Vec<String>,

    /// Environment variables to set when launching the server.
    /// Only valid with stdio servers.
    #[arg(long, value_parser = parse_env_pair, value_name = "KEY=VALUE")]
    pub env: Vec<(String, String)>,
}

#[derive(Debug, clap::Args)]
pub struct AddMcpStreamableHttpArgs {
    /// URL for a streamable HTTP MCP server.
    #[arg(long)]
    pub url: String,

    /// Optional environment variable to read for a bearer token.
    /// Only valid with streamable HTTP servers.
    #[arg(
        long = "bearer-token-env-var",
        value_name = "ENV_VAR",
        requires = "url"
    )]
    pub bearer_token_env_var: Option<String>,
}

#[derive(Debug, clap::Parser)]
pub struct RemoveArgs {
    /// Name of the MCP server configuration to remove.
    pub name: String,
}

impl McpCli {
    pub async fn run(self) -> Result<()> {
        let McpCli {
            config_overrides,
            subcommand,
        } = self;

        match subcommand {
            McpSubcommand::List(args) => run_list(&config_overrides, args)?,
            McpSubcommand::Get(args) => run_get(&config_overrides, args)?,
            McpSubcommand::Add(args) => run_add(&config_overrides, args)?,
            McpSubcommand::Remove(args) => run_remove(&config_overrides, args)?,
        }

        Ok(())
    }
}

fn run_add(config_overrides: &CliConfigOverrides, add_args: AddArgs) -> Result<()> {
    // Validate any provided overrides even though they are not currently applied.
    let overrides = config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;
    // Load config to validate structure if needed (but we primarily use it for future extension).
    let _config = Config::load_with_cli_overrides(overrides, ConfigOverrides::default())
        .context("failed to load configuration")?;

    let AddArgs {
        name,
        transport_args,
    } = add_args;
    validate_server_name(&name)?;

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let mut servers = load_global_mcp_servers(&codex_home)
        .with_context(|| format!("failed to load MCP servers from {}", codex_home.display()))?;

    let transport = match transport_args {
        AddMcpTransportArgs {
            stdio: Some(stdio), ..
        } => {
            let mut parts = stdio.command.into_iter();
            let command = parts.next().ok_or_else(|| anyhow!("command is required"))?;
            let args: Vec<String> = parts.collect();
            let env_map = if stdio.env.is_empty() {
                None
            } else {
                Some(stdio.env.into_iter().collect::<HashMap<_, _>>())
            };
            McpServerTransportConfig::Stdio {
                command,
                args,
                env: env_map,
                env_vars: Vec::new(),
                cwd: None,
            }
        }
        AddMcpTransportArgs {
            streamable_http:
                Some(AddMcpStreamableHttpArgs {
                    url,
                    bearer_token_env_var,
                }),
            ..
        } => McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers: None,
            env_http_headers: None,
            bearer_token: None,
        },
        AddMcpTransportArgs { .. } => bail!("exactly one of --command or --url must be provided"),
    };

    let new_entry = McpServerConfig {
        transport,
        enabled: true,
        startup_timeout_sec: None,
        tool_timeout_sec: None,
    };

    servers.insert(name.clone(), new_entry);

    write_global_mcp_servers(&codex_home, &servers)
        .with_context(|| format!("failed to write MCP servers to {}", codex_home.display()))?;

    println!("Added global MCP server '{name}'.");

    Ok(())
}

fn run_remove(config_overrides: &CliConfigOverrides, remove_args: RemoveArgs) -> Result<()> {
    config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;

    let RemoveArgs { name } = remove_args;

    validate_server_name(&name)?;

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let mut servers = load_global_mcp_servers(&codex_home)
        .with_context(|| format!("failed to load MCP servers from {}", codex_home.display()))?;

    let removed = servers.remove(&name).is_some();

    if removed {
        write_global_mcp_servers(&codex_home, &servers)
            .with_context(|| format!("failed to write MCP servers to {}", codex_home.display()))?;
    }

    if removed {
        println!("Removed global MCP server '{name}'.");
    } else {
        println!("No MCP server named '{name}' found.");
    }

    Ok(())
}

fn run_list(config_overrides: &CliConfigOverrides, list_args: ListArgs) -> Result<()> {
    let overrides = config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;
    let config = Config::load_with_cli_overrides(overrides, ConfigOverrides::default())
        .context("failed to load configuration")?;

    let mut entries: Vec<_> = config.mcp_servers.iter().collect();
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));

    if list_args.json {
        let json_entries: Vec<_> = entries
            .into_iter()
            .map(|(name, cfg)| {
                let transport = match &cfg.transport {
                    McpServerTransportConfig::Stdio {
                        command,
                        args,
                        env,
                        env_vars,
                        cwd,
                    } => serde_json::json!({
                        "type": "stdio",
                        "command": command,
                        "args": args,
                        "env": env,
                        "env_vars": env_vars,
                        "cwd": cwd,
                    }),
                    McpServerTransportConfig::StreamableHttp {
                        url,
                        bearer_token_env_var,
                        http_headers,
                        env_http_headers,
                        bearer_token,
                    } => {
                        serde_json::json!({
                            "type": "streamable_http",
                            "url": url,
                            "bearer_token_env_var": bearer_token_env_var,
                            "http_headers": http_headers,
                            "env_http_headers": env_http_headers,
                            // legacy value if present
                            "bearer_token": bearer_token,
                        })
                    }
                };

                serde_json::json!({
                    "name": name,
                    "enabled": cfg.enabled,
                    "transport": transport,
                    "startup_timeout_sec": cfg
                        .startup_timeout_sec
                        .map(|timeout| timeout.as_secs_f64()),
                    "tool_timeout_sec": cfg
                        .tool_timeout_sec
                        .map(|timeout| timeout.as_secs_f64()),
                })
            })
            .collect();
        let output = serde_json::to_string_pretty(&json_entries)?;
        println!("{output}");
        return Ok(());
    }

    if entries.is_empty() {
        println!("No MCP servers configured yet. Try `codex mcp add my-tool -- my-command`.");
        return Ok(());
    }

    let mut stdio_rows: Vec<[String; 5]> = Vec::new();
    let mut http_rows: Vec<[String; 4]> = Vec::new();

    for (name, cfg) in entries {
        match &cfg.transport {
            McpServerTransportConfig::Stdio {
                command,
                args,
                env,
                env_vars,
                cwd,
            } => {
                let args_display = if args.is_empty() {
                    "-".to_string()
                } else {
                    args.join(" ")
                };
                let env_display = format_env_display(env.as_ref(), env_vars.clone());
                let cwd_display = cwd
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "-".to_string());
                stdio_rows.push([
                    name.clone(),
                    command.clone(),
                    args_display,
                    cwd_display,
                    env_display,
                ]);
            }
            McpServerTransportConfig::StreamableHttp {
                url,
                bearer_token_env_var,
                ..
            } => {
                let env_var = bearer_token_env_var.as_deref().unwrap_or("-").to_string();
                http_rows.push([name.clone(), url.clone(), env_var, cfg.enabled.to_string()]);
            }
        }
    }

    if !stdio_rows.is_empty() {
        let mut widths = [
            "Name".len(),
            "Command".len(),
            "Args".len(),
            "Cwd".len(),
            "Env".len(),
        ];
        for row in &stdio_rows {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(cell.len());
            }
        }

        println!(
            "{:<name_w$}  {:<cmd_w$}  {:<args_w$}  {:<cwd_w$}  {:<env_w$}",
            "Name",
            "Command",
            "Args",
            "Cwd",
            "Env",
            name_w = widths[0],
            cmd_w = widths[1],
            args_w = widths[2],
            cwd_w = widths[3],
            env_w = widths[4],
        );

        for row in &stdio_rows {
            println!(
                "{:<name_w$}  {:<cmd_w$}  {:<args_w$}  {:<cwd_w$}  {:<env_w$}",
                row[0],
                row[1],
                row[2],
                row[3],
                row[4],
                name_w = widths[0],
                cmd_w = widths[1],
                args_w = widths[2],
                cwd_w = widths[3],
                env_w = widths[4],
            );
        }
    }

    if !stdio_rows.is_empty() && !http_rows.is_empty() {
        println!();
    }

    if !http_rows.is_empty() {
        let mut widths = [
            "Name".len(),
            "Url".len(),
            "Bearer Token Env".len(),
            "Enabled".len(),
        ];
        for row in &http_rows {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(cell.len());
            }
        }

        println!(
            "{:<name_w$}  {:<url_w$}  {:<envvar_w$}  {:<enabled_w$}",
            "Name",
            "Url",
            "Bearer Token Env",
            "Enabled",
            name_w = widths[0],
            url_w = widths[1],
            envvar_w = widths[2],
            enabled_w = widths[3],
        );

        for row in &http_rows {
            println!(
                "{:<name_w$}  {:<url_w$}  {:<envvar_w$}  {:<enabled_w$}",
                row[0],
                row[1],
                row[2],
                row[3],
                name_w = widths[0],
                url_w = widths[1],
                envvar_w = widths[2],
                enabled_w = widths[3],
            );
        }
    }

    Ok(())
}

fn run_get(config_overrides: &CliConfigOverrides, get_args: GetArgs) -> Result<()> {
    let overrides = config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;
    let config = Config::load_with_cli_overrides(overrides, ConfigOverrides::default())
        .context("failed to load configuration")?;

    let Some(server) = config.mcp_servers.get(&get_args.name) else {
        bail!("No MCP server named '{name}' found.", name = get_args.name);
    };

    if get_args.json {
        let transport = match &server.transport {
            McpServerTransportConfig::Stdio { command, args, env, env_vars, cwd } => serde_json::json!({
                "type": "stdio",
                "command": command,
                "args": args,
                "env": env,
                "env_vars": env_vars,
                "cwd": cwd,
            }),
            McpServerTransportConfig::StreamableHttp { url, bearer_token_env_var, http_headers, env_http_headers, bearer_token } => serde_json::json!({
                "type": "streamable_http",
                "url": url,
                "bearer_token_env_var": bearer_token_env_var,
                "http_headers": http_headers,
                "env_http_headers": env_http_headers,
                "bearer_token": bearer_token,
            }),
        };
        let output = serde_json::to_string_pretty(&serde_json::json!({
            "name": get_args.name,
            "enabled": server.enabled,
            "transport": transport,
            "startup_timeout_sec": server
                .startup_timeout_sec
                .map(|timeout| timeout.as_secs_f64()),
            "tool_timeout_sec": server
                .tool_timeout_sec
                .map(|timeout| timeout.as_secs_f64()),
        }))?;
        println!("{output}");
        return Ok(());
    }

    println!("{}", get_args.name);
    println!("  enabled: {}", server.enabled);
    match &server.transport {
        McpServerTransportConfig::Stdio {
            command,
            args,
            env,
            env_vars,
            cwd,
        } => {
            println!("  transport: stdio");
            println!("  command: {command}");
            let args_display = if args.is_empty() {
                "-".to_string()
            } else {
                args.join(" ")
            };
            println!("  args: {args_display}");
            let cwd_display = cwd
                .as_ref()
                .map(|p| p.display().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "-".to_string());
            println!("  cwd: {cwd_display}");
            let env_display = format_env_display(env.as_ref(), env_vars.clone());
            println!("  env: {env_display}");
        }
        McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers,
            env_http_headers,
            bearer_token,
        } => {
            println!("  transport: streamable_http");
            println!("  url: {url}");
            let env_var = bearer_token_env_var.as_deref().unwrap_or("-");
            println!("  bearer_token_env_var: {env_var}");
            let headers_display = match http_headers {
                Some(map) if !map.is_empty() => {
                    let mut pairs: Vec<_> = map.iter().collect();
                    pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
                    pairs
                        .into_iter()
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
                _ => "-".to_string(),
            };
            println!("  http_headers: {headers_display}");
            let env_headers_display = match env_http_headers {
                Some(map) if !map.is_empty() => {
                    let mut pairs: Vec<_> = map.iter().collect();
                    pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
                    pairs
                        .into_iter()
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
                _ => "-".to_string(),
            };
            println!("  env_http_headers: {env_headers_display}");
            if bearer_token.is_some() {
                println!("  bearer_token: set (legacy)");
            }
        }
    }
    if let Some(timeout) = server.startup_timeout_sec {
        println!("  startup_timeout_sec: {}", timeout.as_secs_f64());
    }
    if let Some(timeout) = server.tool_timeout_sec {
        println!("  tool_timeout_sec: {}", timeout.as_secs_f64());
    }
    println!("  remove: codex mcp remove {}", get_args.name);

    Ok(())
}

fn parse_env_pair(raw: &str) -> Result<(String, String), String> {
    let mut parts = raw.splitn(2, '=');
    let key = parts
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "environment entries must be in KEY=VALUE form".to_string())?;
    let value = parts
        .next()
        .map(str::to_string)
        .ok_or_else(|| "environment entries must be in KEY=VALUE form".to_string())?;

    Ok((key.to_string(), value))
}

fn validate_server_name(name: &str) -> Result<()> {
    let is_valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');

    if is_valid {
        Ok(())
    } else {
        bail!("invalid server name '{name}' (use letters, numbers, '-', '_')");
    }
}
