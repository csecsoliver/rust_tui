macro_rules! error {
    ($($arg:tt)*) => {
        Err(format!($($arg)*).into())
    };
}
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListState, Paragraph, Wrap},
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::sync::{Arc, RwLock};
use std::{io, sync::OnceLock, thread, time::Duration};
static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();

fn get_client() -> reqwest::blocking::Client {
    CLIENT
        .get_or_init(|| reqwest::blocking::Client::builder().build().unwrap())
        .clone()
}
type ThreadSafeError = Box<dyn std::error::Error + Send + Sync>;

struct JulesApp {
    api_key: String,
    sessions_page: Arc<RwLock<Option<Result<SessionsPage, ThreadSafeError>>>>,
    selected_session: Option<usize>,
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
        let sessions_page = Arc::new(RwLock::new(None));
        let asd = Self {
            api_key,
            sessions_page,
            selected_session: None,
        };
        return asd;
    }
    fn start_fetch<T>(
        &mut self,
        route: String,
        target: Arc<RwLock<Option<Result<T, ThreadSafeError>>>>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: DeserializeOwned + Send + Sync + 'static,
    {
        let key = self.api_key.clone();
        let api_response = target;
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
                    let response = (|| -> Result<T, ThreadSafeError> {
                        let response = client
                            .get(format!("https://jules.googleapis.com/{routestr}"))
                            .header("x-goog-api-key", key)
                            .send();
                        let resres = response?;
                        let text = &resres.text()?;
                        let json = serde_json::from_str(text)?;
                        Ok(json)
                    })();
                    *w = Some(response);
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
    session_list_state: &mut ListState,
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = app.start_fetch(
        format!("v1alpha/sessions?pageSize=50"),
        app.sessions_page.clone(),
    );
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
            match app.selected_session {
                Some(sel) => {



                },
                None => {
                    let header_text = format!("List of sessions (max 50)");
                    let header = Paragraph::new(header_text)
                        .block(Block::default().borders(Borders::ALL).title("Jules TUI"))
                        .style(
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        );
                    f.render_widget(header, chunks[0]);
                    match app.sessions_page.try_read() {
                        Ok(r) => match &*r {
                            Some(Ok(v)) => {
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

                                let sessions = List::new(lines)
                                    .block(Block::default().borders(Borders::ALL).title("Sessions"))
                                    .highlight_style(Style::new().reversed());
                                f.render_stateful_widget(sessions, chunks[1], session_list_state);
                            }
                            Some(Err(e)) => {
                                f.render_widget(
                                    Line::from(Span::styled(
                                        format!("{}", e.to_string()),
                                        Style::default().fg(Color::Red),
                                    )),
                                    chunks[1],
                                );
                                drop(r);
                            }
                            None => {
                                f.render_widget(
                                    Line::from(Span::styled("Loading...", Style::default().fg(Color::Red))),
                                    chunks[1],
                                );
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
                }
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
                        KeyCode::Down | KeyCode::Char('j') => session_list_state.select_next(),
                        KeyCode::Up | KeyCode::Char('k') => session_list_state.select_previous(),
                        KeyCode::Enter => match session_list_state.selected() {
                            Some(s) => {
                                if s % 2 == 0 {
                                    app.selected_session = Some(s / 2);
                                } else {
                                    match app.sessions_page.try_read() {
                                        Ok(r) => {
                                            if let Some(Ok(respon)) = &*r {
                                                match respon.sessions[(s - 1) / 2]["url"].clone() {
                                                    Value::String(url) => {
                                                        let _ = webbrowser::open(&url);
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                        Err(_) => {}
                                    }
                                }
                            }
                            None => {}
                        },
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

    let mut session_list_state: ListState = ListState::default();
    let mut app = JulesApp::new(format!(
        "AQ.Ab8RN6LYT8p7IElcMQbfqD5GvCrHj8pNna_9_WC_fEppVhwqGQ"
    ));
    let result = run_app(&mut app, &mut terminal, &mut session_list_state);
    let _ = disable_raw_mode();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    let _ = terminal.show_cursor();
    result
}
