#![allow(non_camel_case_types)]
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::{Arc, RwLock};
use std::{io, sync::OnceLock, thread, time::Duration};
static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();

fn get_client() -> reqwest::blocking::Client {
    CLIENT
        .get_or_init(|| reqwest::blocking::Client::builder().build().unwrap())
        .clone()
}

struct JulesApp {
    api_key: String,
    api_response: Arc<RwLock<String>>,
}

#[derive(Deserialize)]
struct SessionsPage {
    sessions: Vec<Value>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}
#[derive(Deserialize)]
struct SessionRes {
    name: String,
    id: String,
    title: String,
    state: String,
    url: String,
    #[serde(rename = "createTime")]
    create_time: String,
    #[serde(rename = "updateTime")]
    update_time: String,
}
impl JulesApp {
    fn new(api_key: String) -> Self {
        let api_response = Arc::new(RwLock::new(String::new()));
        let asd = Self {
            api_key,
            api_response,
        };
        return asd;
    }
    fn start_fetch(&mut self, route: String) -> Result<(), Box<dyn std::error::Error>> {
        let key = self.api_key.clone();
        let api_response = self.api_response.clone();
        let routestr = route.clone();
        // match self.api_response.try_write() {
        //     Ok(w) => {
        //         *w = ""
        //     }
        // }

        thread::spawn(move || {
            match api_response.try_write() {
                Ok(mut w) => {
                    let client = get_client();

                    let response = match match client
                        .get(format!("https://jules.googleapis.com/{routestr}"))
                        .header("x-goog-api-key", key)
                        .send()
                    {
                        Ok(r) => r.text(),
                        Err(e) => Err(e),
                    } {
                        Ok(j) => j,
                        Err(e) => {
                            format!("Error: {e}")
                        }
                    };
                    *w = response;
                    drop(w)
                }
                Err(_) => return,
            };
        });
        Ok(())
    }
}
fn run_app(
    app: &mut JulesApp,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    scroll: &mut u16,
) -> Result<(), Box<dyn std::error::Error>> {
    app.start_fetch(format!("v1alpha/sessions?pageSize=50"));
    loop {
        terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(1),
                ])
                .split(area);
            let header_text = format!("Jules TUI");
            let header = Paragraph::new(header_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("List of sessions (max 50)"),
                )
                .style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                );
            f.render_widget(header, chunks[0]);
            match app.api_response.try_read() {
                Ok(r) => match serde_json::from_str::<SessionsPage>(r.as_str()) {
                    Ok(v) => {
                        let mut lines: Vec<Line> = Vec::new();
                        if v.sessions.is_empty() {
                            lines.push(Line::from(Span::styled(
                                "no sessions yet",
                                Style::default().fg(Color::DarkGray),
                            )));
                        }
                        for s in &v.sessions {
                            lines.push(Line::from(Span::styled(
                                format!(
                                    "-> {}",
                                    match s["title"].clone() {
                                        Value::String(s) => s,
                                        _ => String::from("No title"),
                                    }
                                ),
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                            )));
                            lines.push(Line::from(format!(
                                "URL: {}",
                                match s["url"].clone() {
                                    Value::String(s) => s,
                                    _ => String::from("No url"),
                                }
                            )));
                        }

                        let comments = Paragraph::new(lines)
                            .block(Block::default().borders(Borders::ALL).title("Comments"))
                            .wrap(Wrap { trim: false })
                            .scroll((*scroll, 0));
                        f.render_widget(comments, chunks[1]);
                    }
                    Err(e) => {
                        f.render_widget(
                            Line::from(Span::styled(
                                format!("{}: {}", e.to_string(), r),
                                Style::default().fg(Color::Red),
                            )),
                            chunks[1],
                        );
                        drop(r);
                    }
                },
                Err(e) => match e {
                    std::sync::TryLockError::Poisoned(_) => {
                        f.render_widget(
                            Line::from(Span::styled(
                                "the lock is poisoned. the fetcher thread probably paniced.",
                                Style::default().fg(Color::Red),
                            )),
                            chunks[1],
                        );
                    }
                    std::sync::TryLockError::WouldBlock => {
                        f.render_widget(
                            Line::from(Span::styled("Loading...", Style::default().fg(Color::Red))),
                            chunks[1],
                        );
                    }
                },
            }

            let help = Paragraph::new(
                "j/k or arrows to scroll, enter to select, esc to go back, q to quit",
            )
            .style(Style::default().fg(Color::DarkGray));
            f.render_widget(help, chunks[2]);
        })?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1),
                        KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
                        KeyCode::PageDown => *scroll = scroll.saturating_add(10),
                        KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = enable_raw_mode();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut scroll: u16 = 0;
    let mut app = JulesApp::new(format!(
        "AQ.Ab8RN6LYT8p7IElcMQbfqD5GvCrHj8pNna_9_WC_fEppVhwqGQ"
    ));
    let result = run_app(&mut app, &mut terminal, &mut scroll);
    let _ = disable_raw_mode();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    let _ = terminal.show_cursor();
    result
}
