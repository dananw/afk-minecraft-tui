use anyhow::{Context, Result};
use crate::core::Theme;

#[derive(Clone, Debug)]
pub struct Args {
    pub server: String,
    pub username: String,
    pub email: Option<String>,
    pub login_password: Option<String>,
    pub command: Vec<String>,
    pub chat: Option<String>,
    pub reconnect_delay_seconds: u64,
    pub view_distance: u8,
    pub config_file: Option<String>,
    pub no_tui: bool,
    pub theme: Theme,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            server: "localhost:25565".to_string(),
            username: "afk_bot".to_string(),
            email: None,
            login_password: None,
            command: Vec::new(),
            chat: None,
            reconnect_delay_seconds: 5,
            view_distance: 8,
            config_file: None,
            no_tui: false,
            theme: Theme::default(),
        }
    }
}

impl Args {
    pub fn parse(mut raw_args: impl Iterator<Item = String>) -> Result<Self> {
        let mut args = Self::default();

        while let Some(arg) = raw_args.next() {
            match arg.as_str() {
                "--server" | "-s" => {
                    args.server = next_arg(&mut raw_args, "--server")?;
                }
                "--username" | "-u" => {
                    args.username = next_arg(&mut raw_args, "--username")?;
                }
                "--email" | "-e" => {
                    args.email = Some(next_arg(&mut raw_args, "--email")?);
                }
                "--password" | "-p" => {
                    args.login_password = Some(next_arg(&mut raw_args, "--password")?);
                }
                "--login-password" => {
                    args.login_password = Some(next_arg(&mut raw_args, "--login-password")?);
                }
                "--command" => {
                    args.command.push(next_arg(&mut raw_args, "--command")?);
                }
                "--chat" => {
                    args.chat = Some(next_arg(&mut raw_args, "--chat")?);
                }
                "--reconnect-delay" | "-r" => {
                    args.reconnect_delay_seconds = next_arg(&mut raw_args, "--reconnect-delay")?
                        .parse()
                        .context("--reconnect-delay must be a number")?;
                }
                "--view-distance" | "-v" => {
                    args.view_distance = next_arg(&mut raw_args, "--view-distance")?
                        .parse()
                        .context("--view-distance must be a number")?;
                }
                "--config" | "-c" => {
                    args.config_file = Some(next_arg(&mut raw_args, "--config")?);
                }
                "--no-tui" => {
                    args.no_tui = true;
                }
                "--theme" => {
                    let theme_name = next_arg(&mut raw_args, "--theme")?;
                    args.theme = match theme_name.as_str() {
                        "light" => Theme::light(),
                        "dark" | _ => Theme::dark(),
                    };
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                other => {
                    anyhow::bail!("Unknown argument: {}", other);
                }
            }
        }

        Ok(args)
    }

    pub fn merge_with_config(&mut self, config: &crate::core::Config) {
        if self.server == Self::default().server {
            self.server = config.server.clone();
        }
        if self.username == Self::default().username {
            self.username = config.username.clone();
        }
        if self.email.is_none() {
            self.email = config.email.clone();
        }
        if self.login_password.is_none() {
            self.login_password = config.password.clone();
        }
        if self.command.is_empty() && !config.auto_commands.is_empty() {
            self.command = config.auto_commands.clone();
        }
        if self.view_distance == Self::default().view_distance {
            self.view_distance = config.view_distance;
        }
        if self.reconnect_delay_seconds == Self::default().reconnect_delay_seconds {
            self.reconnect_delay_seconds = config.reconnect_delay;
        }
        self.theme = config.theme.to_theme();
    }
}

fn next_arg(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    args.next()
        .with_context(|| format!("Value for {} not provided", flag))
}

fn print_usage() {
    println!("Minecraft AFK TUI - Terminal Bot Client");
    println!();
    println!("Usage:");
    println!("  mc-bot [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -s, --server <ADDR>          Server address (default: localhost:25565)");
    println!("  -u, --username <NAME>        Username for offline mode");
    println!("  -e, --email <EMAIL>          Microsoft account email");
    println!("  -p, --password <PASS>        Password for cracked server auth");
    println!("      --command <CMD>          Auto-send command on connect (can use multiple)");
    println!("      --chat <TEXT>            Auto-send chat on connect");
    println!("  -r, --reconnect-delay <SEC>  Reconnect delay (default: 5)");
    println!("  -v, --view-distance <CHUNKS> View distance (default: 8)");
    println!("  -c, --config <FILE>          Config file path");
    println!("      --no-tui                 Run without TUI (simple mode)");
    println!("      --theme <NAME>           Color theme: dark or light (default: dark)");
    println!("  -h, --help                   Show this help");
    println!();
    println!("Examples:");
    println!("  # Offline mode with auth");
    println!("  mc-bot -s mc.server.com -u MyBot -p mypassword");
    println!();
    println!("  # Microsoft account");
    println!("  mc-bot -s mc.server.com -e user@example.com");
    println!();
    println!("  # With auto commands");
    println!("  mc-bot -s mc.server.com -u Bot --command '/login pass' --command '/warp afk'");
}
