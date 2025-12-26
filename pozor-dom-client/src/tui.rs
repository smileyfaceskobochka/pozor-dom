use futures_util::SinkExt;
use tokio_tungstenite::tungstenite::protocol::Message;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde_json::json;
use chrono::Local;
use std::io;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

#[derive(Clone)]
pub struct AppState {
    pub messages: Vec<String>,
    pub input: String,
    pub input_history: Vec<String>,
    pub history_index: Option<usize>,
    pub connected: bool,
    pub server_url: String,
    pub device_id: Option<String>,
    pub channel: Option<String>,
}

impl AppState {
    pub fn new(server_url: String) -> Self {
        Self {
            messages: vec!["Pozor-dom Client starting...".to_string()],
            input: String::new(),
            input_history: Vec::new(),
            history_index: None,
            connected: false,
            server_url,
            device_id: None,
            channel: None,
        }
    }

    pub fn add_message(&mut self, msg: String) {
        let timestamp = Local::now().format("%H:%M:%S");
        self.messages.push(format!("[{}] {}", timestamp, msg));
        // Keep only last 100 messages
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
    }
}

pub async fn run_tui(
    app_state: Arc<Mutex<AppState>>,
    ws_write: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>
        >,
        Message
    >
) -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, app_state.clone(), ws_write).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app_state: Arc<Mutex<AppState>>,
    mut ws_write: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>
        >,
        Message
    >
) -> io::Result<()> {
    loop {
        {
            let state = app_state.lock().await.clone();
            terminal.draw(|f| ui(f, &state))?;
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Enter => {
                        let mut state = app_state.lock().await;
                        if !state.input.trim().is_empty() {
                            let input = state.input.clone();

                            // Add to history
                            state.input_history.push(input.clone());
                            if state.input_history.len() > 50 {
                                state.input_history.remove(0);
                            }
                            state.history_index = None;

                            state.input.clear();

                            // Handle special commands
                            if input.starts_with("/device ") {
                                let parts: Vec<&str> = input.split_whitespace().collect();
                                if parts.len() >= 3 {
                                    state.device_id = Some(parts[1].to_string());
                                    state.channel = Some(parts[2].to_string());
                                    state.add_message(format!("Set target: {} on {} channel", parts[1], parts[2]));
                                }
                            } else if input.starts_with("/send ") {
                                let action = input.strip_prefix("/send ").unwrap_or("");
                                let has_device = state.device_id.is_some() && state.channel.is_some();

                                if has_device {
                                    let device_id = state.device_id.as_ref().unwrap().clone();
                                    let channel = state.channel.as_ref().unwrap().clone();

                                    let command = json!({
                                        "type": "command",
                                        "device_id": device_id,
                                        "channel": channel,
                                        "action": action,
                                        "timestamp": Local::now().to_rfc3339()
                                    });

                                    state.add_message(format!("Sending command: {} to {} ({})", action, device_id, channel));

                                    if let Err(e) = ws_write.send(Message::Text(command.to_string().into())).await {
                                        state.add_message(format!("Failed to send command: {}", e));
                                    }
                                } else {
                                    state.add_message("Error: Set device first with /device <id> <channel>. Use: wifi, ble, zigbee".to_string());
                                }
                            } else {
                                // Send as regular message
                                state.add_message(format!("Sending: {}", input));
                                if let Err(e) = ws_write.send(Message::Text(input.into())).await {
                                    state.add_message(format!("Failed to send message: {}", e));
                                }
                            }
                        }
                    }
                    KeyCode::Up => {
                        let mut state = app_state.lock().await;
                        if !state.input_history.is_empty() {
                            let new_index = state.history_index
                                .map(|i| if i > 0 { i - 1 } else { state.input_history.len() - 1 })
                                .unwrap_or(state.input_history.len() - 1);
                            state.history_index = Some(new_index);
                            state.input = state.input_history[new_index].clone();
                        }
                    }
                    KeyCode::Down => {
                        let mut state = app_state.lock().await;
                        if let Some(current_index) = state.history_index {
                            if current_index < state.input_history.len() - 1 {
                                let new_index = current_index + 1;
                                state.history_index = Some(new_index);
                                state.input = state.input_history[new_index].clone();
                            } else {
                                state.history_index = None;
                                state.input.clear();
                            }
                        }
                    }
                    KeyCode::Char(c) => {
                        let mut state = app_state.lock().await;
                        state.input.push(c);
                        state.history_index = None; // Reset history navigation
                    }
                    KeyCode::Backspace => {
                        let mut state = app_state.lock().await;
                        state.input.pop();
                        state.history_index = None; // Reset history navigation
                    }
                    KeyCode::Esc => {
                        return Ok(());
                    }
                    KeyCode::Tab => {
                        let mut state = app_state.lock().await;
                        if state.input.starts_with("/device ") {
                            // Auto-complete device commands
                            if state.input == "/device " {
                                state.input = "/device device-wifi-001 wifi".to_string();
                            }
                        } else if state.input.starts_with("/send ") {
                            // Auto-complete send commands
                            if state.input == "/send " {
                                state.input = "/send get_temperature".to_string();
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame, state: &AppState) {
    let size = f.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(1),    // Messages
            Constraint::Length(3), // Input
            Constraint::Length(1), // Help
        ])
        .split(size);

    // Create owned strings to avoid temporary value issues
    let device_display = state.device_id.as_ref().map(|s| s.as_str()).unwrap_or("None");
    let channel_display = state.channel.as_ref().map(|s| s.as_str()).unwrap_or("None");

    // Header with more information
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("🏠 Pozor-dom Client", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" - "),
            Span::styled(&state.server_url, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("📊 Status: "),
            if state.connected {
                Span::styled("🟢 Connected", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            } else {
                Span::styled("🔴 Connecting...", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            },
            Span::raw(" | 🎯 Device: "),
            Span::styled(device_display, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::raw(" | 📡 Channel: "),
            Span::styled(channel_display, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::raw("📝 Messages: "),
            Span::styled(state.messages.len().to_string(), Style::default().fg(Color::Blue)),
            Span::raw(" | ⌨️  ↑↓ History | Tab Auto-complete | ESC Quit"),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title("🎮 Control Panel"));
    f.render_widget(header, chunks[0]);

    // Messages
    let messages: Vec<ListItem> = state
        .messages
        .iter()
        .map(|msg| {
            ListItem::new(msg.as_str())
        })
        .collect();

    let messages_list = List::new(messages)
        .block(Block::default().borders(Borders::ALL).title("Messages"))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    f.render_widget(messages_list, chunks[1]);

    // Input
    let input = Paragraph::new(state.input.as_str())
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input, chunks[2]);

    // Help
    let help = Paragraph::new("Commands: /device <id> <channel> | /send <action> | ESC to quit")
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .wrap(Wrap { trim: true });
    f.render_widget(help, chunks[3]);
}
