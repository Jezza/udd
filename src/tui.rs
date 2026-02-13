use crate::{Args, utils};
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
    MouseEventKind,
};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{event, execute};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::prelude::{Color, Line, Span, Style, Stylize};
use ratatui::widgets::{
    Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use ratatui::{Frame, Terminal};
use std::io;
use std::io::{ErrorKind, stdout};
use std::net::UdpSocket;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

mod format;
mod parse;

static MSG_ID_COUNTER: AtomicU16 = AtomicU16::new(1);

fn next_msg_id() -> u16 {
    MSG_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Copy, PartialEq)]
enum InputMode {
    Auto,
    Text,
    Hex,
    Mqtt,
}

struct LogEntry {
    display: String,
    style: Style,
    payload: Option<(InputMode, Vec<u8>)>, // Original mode + data for replay
}

struct App {
    socket: UdpSocket,
    input: String,
    input_mode: InputMode,
    log: Vec<LogEntry>,
    log_area: Rect,
    scroll_offset: usize,
    running: bool,
}

impl App {
    fn new(socket: UdpSocket) -> io::Result<Self> {
        Ok(Self {
            socket,
            input: String::new(),
            input_mode: InputMode::Auto,
            log: vec![LogEntry {
                display: "Ready. Tab=mode, Enter=send, Esc=quit, Click=replay".into(),
                style: Style::default().dim(),
                payload: None,
            }],
            log_area: Rect::default(),
            scroll_offset: 0,
            running: true,
        })
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
        // Auto-scroll to bottom
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

        let data = match mode {
            InputMode::Auto => match parse::parse_mqtt_command(&input).map(|f| f.encode()) {
                Ok(value) => Ok(value),
                Err(err) if mode == InputMode::Mqtt => Err(err),
                Err(_err) => match utils::parse_hex(&input) {
                    Ok(value) => Ok(value),
                    Err(err) if mode == InputMode::Hex => Err(err),
                    Err(_err) => Ok(utils::parse_text_with_escapes(&input)),
                },
            },
            InputMode::Text => Ok(utils::parse_text_with_escapes(&input)),
            InputMode::Hex => utils::parse_hex(&input),
            InputMode::Mqtt => parse::parse_mqtt_command(&input).map(|frame| frame.encode()),
        };

        let data = match data {
            Ok(data) => data,
            Err(err) => {
                self.log_error(err);
                return;
            }
        };

        let n = match self.socket.send(&data) {
            Ok(n) => n,
            Err(e) => {
                self.log_error(format!("Send failed: {}", e));
                return;
            }
        };

        let mode_str = match mode {
            InputMode::Auto => "AUTO",
            InputMode::Text => "TXT",
            InputMode::Hex => "HEX",
            InputMode::Mqtt => "MQTT",
        };

        let display = format::format(&data);

        self.log_msg(
            format!("→ [{}] {} bytes: {}", mode_str, n, display),
            Style::default().fg(Color::Cyan),
            Some((mode, data)),
        );
    }

    fn cycle_mode(&mut self) {
        self.input_mode = match self.input_mode {
            InputMode::Auto => InputMode::Mqtt,
            InputMode::Mqtt => InputMode::Text,
            InputMode::Text => InputMode::Hex,
            InputMode::Hex => InputMode::Auto,
        };
    }

    // fn drain_errors2(&mut self) {
    //     let mut buf = [0u8; 1];
    //     loop {
    //         match self.socket.recv(&mut buf) {
    //             Ok(_) => {
    //                 self.log_msg(
    //                     "← Received unexpected data".into(),
    //                     Style::default().fg(Color::Yellow),
    //                     None,
    //                 );
    //             }
    //             Err(e) if e.kind() == ErrorKind::WouldBlock => break,
    //             Err(e) if e.kind() == ErrorKind::ConnectionRefused => {
    //                 self.log_msg(
    //                     "✗ ICMP: Connection refused (port unreachable)".into(),
    //                     Style::default().fg(Color::Red),
    //                     None,
    //                 );
    //             }
    //             Err(e) => {
    //                 self.log_msg(
    //                     format!("✗ ICMP: {}", e),
    //                     Style::default().fg(Color::Red),
    //                     None,
    //                 );
    //                 break;
    //             }
    //         }
    //     }
    // }

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
    let socket = UdpSocket::bind(&args.bind)?;
    socket.connect(&args.target)?;
    socket.set_nonblocking(true)?;

    let mut app = App::new(socket)?;

    let logs = Arc::new(Mutex::new(vec![]));

    if let Ok(socket) = app.socket.try_clone() {
        std::thread::spawn({
            let socket = socket;
            let logs = logs.clone();

            move || {
                let mut buffer = [0u8; 4096];
                loop {
                    let n = match socket.recv(&mut buffer) {
                        Ok(n) => n,
                        Err(err) if err.kind() == ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(1000));
                            continue;
                        }
                        Err(err) => {
                            // app.log_error(format!("{}", err));
                            continue;
                        }
                    };

                    let buffer = &buffer[0..n];

                    {
                        let mut logs = logs.lock().unwrap();

                        logs.push(buffer.to_vec());
                    }
                }
            }
        });
    }

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let target = &args.target;

    while app.running {
        terminal.draw(|f| draw(f, &mut app, target))?;

        {
            let mut incoming = logs.lock().unwrap();
            for raw in incoming.drain(..) {
                let display = format::format(&raw);
                app.log_msg(
                    format!("← {} bytes: {}", raw.len(), display),
                    Style::default().fg(Color::Green),
                    Some((app.input_mode, raw)),
                );
            }
        }

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
