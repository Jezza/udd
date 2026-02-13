use std::io::{BufRead, Write};
use std::net::UdpSocket;

pub fn run(args: &crate::Args) -> std::io::Result<()> {
    println!("CLI mode - use --mode tui for TUI");
    let socket = UdpSocket::bind(&args.bind)?;
    socket.connect(&args.target)?;

    println!("UDP sender ready → {}", args.target);
    println!("Commands:");
    println!("  text <message>     Send text");
    println!("  hex <bytes>        Send hex (e.g., hex deadbeef)");
    println!("  file <path>        Send file contents");
    println!("  quit               Exit\n");

    let stdin = std::io::stdin();
    loop {
        print!("> ");
        std::io::stdout().flush()?;

        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let (cmd, arg) = line.split_once(' ').unwrap_or((line, ""));

        match cmd {
            "text" => {
                let sent = socket.send(arg.as_bytes())?;
                println!("Sent {} bytes", sent);
            }
            "hex" => match crate::utils::parse_hex(arg) {
                Ok(data) => {
                    let sent = socket.send(&data)?;
                    println!("Sent {} bytes", sent);
                }
                Err(e) => eprintln!("Invalid hex: {}", e),
            },
            "file" => match std::fs::read(arg) {
                Ok(data) => {
                    let sent = socket.send(&data)?;
                    println!("Sent {} bytes from file", sent);
                }
                Err(e) => eprintln!("File error: {}", e),
            },
            "quit" | "exit" => break,
            _ => eprintln!("Unknown command: {}", cmd),
        }
    }

    Ok(())
}
