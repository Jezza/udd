mod cli;
mod tui;
mod utils;

#[derive(clap::Parser)]
#[command(name = "udd", about = "UDP client with single-shot CLI and TUI")]
struct Args {
    target: String,
    #[arg(short, long, default_value = "0.0.0.0:0")]
    bind: String,
    #[arg(long)]
    tui: bool,
    #[arg(long, value_enum, default_value_t = InputMode::Auto)]
    mode: InputMode,
    #[arg(
        value_name = "COMMAND",
        trailing_var_arg = true,
        allow_hyphen_values = true
    )]
    command: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum InputMode {
    Auto,
    Text,
    Hex,
    Mqtt,
}

impl InputMode {
    pub(crate) fn short_label(self) -> &'static str {
        match self {
            InputMode::Auto => "AUTO",
            InputMode::Text => "TXT",
            InputMode::Hex => "HEX",
            InputMode::Mqtt => "MQTT",
        }
    }
}

fn main() -> std::io::Result<()> {
    let args: Args = clap::Parser::parse();
    match args.tui {
        true => tui::run(&args),
        false => cli::run(&args),
    }
}
