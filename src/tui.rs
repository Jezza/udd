use crate::{Args, utils};
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind,
};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{event, execute};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Color, Line, Span, Style, Stylize};
use ratatui::widgets::{
    Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use ratatui::{Frame, Terminal};
use std::io;
use std::io::{ErrorKind, stdout};
use std::net::UdpSocket;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::Duration;

use crate::InputMode;

mod format;
mod parse;

static MSG_ID_COUNTER: AtomicU16 = AtomicU16::new(1);

fn next_msg_id() -> u16 {
    MSG_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

struct LogEntry {
    display: String,
    style: Style,
    payload: Option<(InputMode, Vec<u8>)>, // Original mode + data for replay
}

struct App {
    tx: Sender<NetCommand>,
    rx: Receiver<NetEvent>,
    input: String,
    input_mode: InputMode,
    log: Vec<LogEntry>,
    log_area: Rect,
    scroll_offset: usize,
    running: bool,
}

enum NetCommand {
    Send { mode: InputMode, input: String },
    Shutdown,
}

enum NetEvent {
    Sent {
        mode: InputMode,
        data: Vec<u8>,
        sent: usize,
    },
    Received(Vec<u8>),
    Error(String),
}

pub(crate) fn parse_payload(mode: InputMode, input: &str) -> Result<(InputMode, Vec<u8>), String> {
    match mode {
        InputMode::Auto => {
            if let Ok(frame) = parse::parse_mqtt_command(input) {
                return Ok((InputMode::Mqtt, frame.encode()));
            }
            if let Ok(hex) = utils::parse_hex(input) {
                return Ok((InputMode::Hex, hex));
            }
            Ok((InputMode::Text, utils::parse_text_with_escapes(input)))
        }
        InputMode::Mqtt => {
            parse::parse_mqtt_command(input).map(|frame| (InputMode::Mqtt, frame.encode()))
        }
        InputMode::Hex => utils::parse_hex(input).map(|hex| (InputMode::Hex, hex)),
        InputMode::Text => Ok((InputMode::Text, utils::parse_text_with_escapes(input))),
    }
}

fn run_network_thread(
    bind: String,
    target: String,
    rx_cmd: Receiver<NetCommand>,
    tx_evt: Sender<NetEvent>,
) {
    let socket = match UdpSocket::bind(&bind) {
        Ok(socket) => socket,
        Err(err) => {
            let _ = tx_evt.send(NetEvent::Error(format!("Bind failed: {}", err)));
            return;
        }
    };

    if let Err(err) = socket.connect(&target) {
        let _ = tx_evt.send(NetEvent::Error(format!("Connect failed: {}", err)));
        return;
    }

    if let Err(err) = socket.set_nonblocking(true) {
        let _ = tx_evt.send(NetEvent::Error(format!(
            "Failed to set nonblocking: {}",
            err
        )));
        return;
    }

    let mut buffer = [0u8; 4096];
    loop {
        loop {
            let (mode, data) = match rx_cmd.try_recv() {
                Ok(NetCommand::Send { mode, input }) => match parse_payload(mode, &input) {
                    Ok(data) => data,
                    Err(err) => {
                        if tx_evt.send(NetEvent::Error(err)).is_err() {
                            return;
                        }
                        continue;
                    }
                },
                Ok(NetCommand::Shutdown) => return,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            };

            match socket.send(&data) {
                Ok(sent) => {
                    if tx_evt.send(NetEvent::Sent { mode, data, sent }).is_err() {
                        return;
                    }
                }
                Err(err) => {
                    if tx_evt
                        .send(NetEvent::Error(format!("Send failed: {}", err)))
                        .is_err()
                    {
                        return;
                    }
                }
            };
        }

        match socket.recv(&mut buffer) {
            Ok(n) => {
                if tx_evt
                    .send(NetEvent::Received(buffer[..n].to_vec()))
                    .is_err()
                {
                    return;
                }
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => {}
            Err(err) if err.kind() == ErrorKind::ConnectionRefused => {
                if tx_evt
                    .send(NetEvent::Error(
                        "ICMP: Connection refused (port unreachable)".to_string(),
                    ))
                    .is_err()
                {
                    return;
                }
            }
            Err(err) => {
                if tx_evt
                    .send(NetEvent::Error(format!("Receive failed: {}", err)))
                    .is_err()
                {
                    return;
                }
            }
        }

        std::thread::sleep(Duration::from_millis(25));
    }
}

impl App {
    fn new(tx: Sender<NetCommand>, rx: Receiver<NetEvent>) -> Self {
        Self {
            tx,
            rx,
            input: String::new(),
            input_mode: InputMode::Auto,
            log: vec![LogEntry {
                display: "Ready. Tab=mode, Enter=send, Esc=quit".into(),
                style: Style::default().dim(),
                payload: None,
            }],
            log_area: Rect::default(),
            scroll_offset: 0,
            running: true,
        }
    }

    fn log_error(&mut self, msg: impl Into<String>) {
        self.log_msg(
            format!("✗ {}", msg.into()),
            Style::default().fg(Color::Red),
            None,
        );
        self.input.clear();
    }

    fn log_msg(&mut self, display: String, style: Style, payload: Option<(InputMode, Vec<u8>)>) {
        self.log.push(LogEntry {
            display,
            style,
            payload,
        });

        let visible = self.log_area.height.saturating_sub(2) as usize;
        if self.log.len() > visible {
            self.scroll_offset = self.log.len() - visible;
        }
    }

    fn send(&mut self) {
        let input = std::mem::take(&mut self.input);

        if input.is_empty() {
            return;
        }

        let mode = self.input_mode;

        if let Err(err) = self.tx.send(NetCommand::Send { mode, input }) {
            self.log_error(format!("Network thread unavailable: {}", err));
            self.running = false;
            return;
        }
    }

    fn on_sent(&mut self, mode: InputMode, data: Vec<u8>, n: usize) {
        let display = format::format_for_mode(mode, &data);

        self.log_msg(
            format!("→ [{}] {} bytes: {}", mode.short_label(), n, display),
            Style::default().fg(Color::Cyan),
            Some((mode, data)),
        );
    }

    fn drain_net_events(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(NetEvent::Sent { mode, data, sent }) => self.on_sent(mode, data, sent),
                Ok(NetEvent::Received(raw)) => {
                    let mode = self.input_mode;
                    let display = format::format_for_mode(mode, &raw);
                    self.log_msg(
                        format!("← {} bytes: {}", raw.len(), display),
                        Style::default().fg(Color::Green),
                        Some((mode, raw)),
                    );
                }
                Ok(NetEvent::Error(err)) => {
                    self.log_msg(format!("✗ {}", err), Style::default().fg(Color::Red), None);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.log_error("Network thread disconnected");
                    self.running = false;
                    break;
                }
            }
        }
    }

    fn cycle_mode(&mut self) {
        self.input_mode = match self.input_mode {
            InputMode::Auto => InputMode::Text,
            InputMode::Text => InputMode::Hex,
            InputMode::Hex => InputMode::Mqtt,
            InputMode::Mqtt => InputMode::Auto,
        };
    }

    fn scroll(&mut self, delta: i16) {
        let visible = self.log_area.height.saturating_sub(2) as usize;
        let max_scroll = self.log.len().saturating_sub(visible);

        if delta < 0 {
            self.scroll_offset = self
                .scroll_offset
                .saturating_sub(delta.unsigned_abs() as usize);
        } else {
            self.scroll_offset = (self.scroll_offset + delta as usize).min(max_scroll);
        }
    }
}

pub fn run(args: &Args) -> io::Result<()> {
    let (tx_cmd, rx_cmd) = mpsc::channel::<NetCommand>();
    let (tx_evt, rx_evt) = mpsc::channel::<NetEvent>();
    let bind = args.bind.clone();
    let target = args.target.clone();
    let network_thread =
        std::thread::spawn(move || run_network_thread(bind, target, rx_cmd, tx_evt));

    let mut app = App::new(tx_cmd, rx_evt);

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let target = &args.target;

    while app.running {
        app.drain_net_events();
        terminal.draw(|f| draw(f, &mut app, target))?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Esc => app.running = false,
                KeyCode::Tab => app.cycle_mode(),
                KeyCode::Enter => app.send(),
                KeyCode::Backspace => {
                    app.input.pop();
                }
                KeyCode::Char(c) => app.input.push(c),
                _ => {}
            },
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => app.scroll(-3),
                MouseEventKind::ScrollDown => app.scroll(3),
                _ => {}
            },
            _ => {}
        }
    }

    let _ = app.tx.send(NetCommand::Shutdown);
    let _ = network_thread.join();

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

fn draw(f: &mut Frame, app: &mut App, target: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(f.area());

    // Store log area for click detection
    app.log_area = chunks[0];

    // Log with scrolling
    let visible_height = chunks[0].height.saturating_sub(2) as usize;
    let items: Vec<ListItem> = app
        .log
        .iter()
        .skip(app.scroll_offset)
        .take(visible_height)
        .map(|e| {
            let style = if e.payload.is_some() {
                e.style.underlined() // Indicate clickable
            } else {
                e.style
            };
            ListItem::new(e.display.as_str()).style(style)
        })
        .collect();

    let log = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Log (click to replay, scroll to navigate)"),
    );
    f.render_widget(log, chunks[0]);

    // Scrollbar
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
    let mut scrollbar_state = ScrollbarState::new(app.log.len()).position(app.scroll_offset);
    f.render_stateful_widget(scrollbar, chunks[0], &mut scrollbar_state);

    let (mode_str, mode_style) = match app.input_mode {
        InputMode::Auto => ("[AUTO]", Style::default().fg(Color::Blue).bold()),
        InputMode::Text => ("[TEXT]", Style::default().fg(Color::Green).bold()),
        InputMode::Hex => ("[HEX] ", Style::default().fg(Color::Magenta).bold()),
        InputMode::Mqtt => ("[MQTT]", Style::default().fg(Color::Yellow).bold()),
    };

    let line = Line::from(vec![
        Span::raw(" Target: "),
        Span::styled(target, Style::default().fg(Color::Cyan)),
        Span::raw(" │ Mode: "),
        Span::styled(mode_str, mode_style),
        Span::raw(" (tab to cycle)"),
    ]);

    let input = Paragraph::new(app.input.as_str())
        .block(Block::default().borders(Borders::ALL).title(line));
    f.render_widget(input, chunks[1]);

    f.set_cursor_position((chunks[1].x + app.input.len() as u16 + 1, chunks[1].y + 1));
}
