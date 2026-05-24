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
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph, canvas::Line},
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::{Arc, RwLock};
use std::{io, ops::DerefMut, sync::OnceLock, thread, time::Duration};
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
    next_page_token: String,
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
    app: JulesApp,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    scroll: &mut u16,
) -> Result<(), Box<dyn std::error::Error>> {
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
            let header_text = format!("asd");
            let header = Paragraph::new(header_text)
                .block(Block::default().borders(Borders::ALL).title("pu"))
                .style(
                    Style::default()
                        .fg(ratatui::style::Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                );
            f.render_widget(header, chunks[0]);

            let mut lines: Vec<Line> = Vec::new();
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
    let app = JulesApp::new(format!("asd"));
    let result = run_app(app, &mut terminal, &mut scroll);
    let _ = disable_raw_mode();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    let _ = terminal.show_cursor();
    result
}
