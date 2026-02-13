mod cli;
mod mqtt;
mod tui;
mod utils;

#[derive(clap::Parser)]
#[command(name = "udd", about = "Interactive NMQTT/UDP cli")]
struct Args {
    target: String,
    #[arg(short, long, default_value = "0.0.0.0:0")]
    bind: String,
    #[arg(long)]
    tui: bool,
}

fn main() -> std::io::Result<()> {
    let args: Args = clap::Parser::parse();
    match args.tui {
        true => tui::run(&args),
        false => cli::run(&args),
    }
}
