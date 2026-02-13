use std::io::{Error, ErrorKind};
use std::net::UdpSocket;

pub fn run(args: &crate::Args) -> std::io::Result<()> {
    let command = args.command.join(" ");
    let command = command.trim();
    if command.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "CLI requires a command. Example: udd <target> --mode mqtt connect id1",
        ));
    }

    let (mode, payload) = crate::tui::parse_payload(args.mode, command)
        .map_err(|err| Error::new(ErrorKind::InvalidInput, err))?;

    let label = mode.short_label();

    let socket = UdpSocket::bind(&args.bind)?;
    socket.connect(&args.target)?;
    let sent = socket.send(&payload)?;
    println!("→ [{}] sent {} bytes to {}", label, sent, args.target);
    Ok(())
}
