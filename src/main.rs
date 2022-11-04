use itertools::Itertools;
use std::error::Error;

use std::io::{self, stdout};

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use tui::backend::{Backend, CrosstermBackend};
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans, Text};
use tui::widgets::{Block, Borders, List, ListItem, Paragraph, Tabs};
use tui::{Frame, Terminal};
use unicode_width::UnicodeWidthStr;

mod server;
use server::Server;

enum InputMode {
    Normal,
    Editing,
}

/// App holds the state of the application
struct App {
    /// Current value of the input box
    input: String,
    /// Current input mode
    input_mode: InputMode,
    // Current scroll amount
    scroll: usize,
    // The current chat that is being viewed
    current_chat: Option<usize>,
    // The initialized server
    server: Server,
}

// TODO: This should have a reference to the TCP streams and the associated messages for each connection
impl App {
    fn new() -> App {
        App {
            input: String::new(),
            input_mode: InputMode::Normal,
            scroll: 0,
            current_chat: None,
            server: Server::new(),
        }
    }
}

impl App {
    fn next(&mut self) {
        let conns = self.server.connections.lock().unwrap();
        if conns.len() > 0 {
            self.current_chat = Some((self.current_chat.unwrap_or(0) + 1) % conns.len())
        }
        drop(conns);
    }

    fn previous(&mut self) {
        if let Some(n) = self.current_chat {
            if n > 0 {
                self.current_chat = Some(n - 1);
            } else {
                let conns = self.server.connections.lock().unwrap();
                self.current_chat = Some(conns.len() - 1);
            }
        }
    }
}

impl App {
    fn scroll_up(&mut self) {
        if self.scroll > 0 {
            self.scroll -= 1;
        }
    }
    fn scroll_down(&mut self) {
        if self.scroll
            < self
                .server
                .get_messages(self.current_chat.unwrap_or(0))
                .iter()
                .len()
        {
            self.scroll += 1;
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize the app state
    let app = App::new();

    // Star the server to listen for incoming connections
    app.server.listen();

    // Start the tui
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // The the tui given the terminal and app state
    let res = run_app(&mut terminal, app);

    // Graceful shutdown of tui
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

/*
This function draws the terminal and registers events happening within the terminal.
It modifies the state of the app given which event happens
*/
fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        // If at least one connection exists, set the current chat to that connection token value
        if 

        if let Event::Key(key) = event::read()? {
            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('e') => {
                        app.input_mode = InputMode::Editing;
                    }
                    KeyCode::Char('q') => {
                        return Ok(());
                    }
                    KeyCode::Right => app.next(),
                    KeyCode::Left => app.previous(),
                    KeyCode::Up => app.scroll_up(),
                    KeyCode::Down => app.scroll_down(),
                    _ => {}
                },
                InputMode::Editing => match key.code {
                    KeyCode::Enter => {
                        if let Some(n) = app.current_chat {
                            app.server.send_message(n + 1, app.input.drain(..).as_str())
                        }
                    }
                    KeyCode::Char(c) => {
                        app.input.push(c);
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::Esc => {
                        app.input_mode = InputMode::Normal;
                    }
                    _ => {}
                },
            }
        }
    }
}

// This function renders the UI components given a frame and the state of the app
fn ui<B: Backend>(f: &mut Frame<B>, app: &App) {
    // Draw the tabs
    let chunks = Layout::default()
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(f.size());

    let connections = app.server.connections.lock().unwrap();
    let titles = connections
        .keys()
        .sorted()
        .map(|t| format!("{}", t))
        .map(|t| Spans::from(Span::styled(t, Style::default().fg(Color::Green))))
        .collect();
    drop(connections);

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("Connections"))
        .select(app.current_chat.unwrap_or(0))
        .style(Style::default().fg(Color::Cyan))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::Black),
        );
    f.render_widget(tabs, chunks[0]);

    if let Some(n) = app.current_chat {
        draw_chat(f, app, n, chunks[1])
    }
}

fn draw_chat<B: Backend>(f: &mut Frame<B>, app: &App, current_chat: usize, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
        .split(area);

    // Render the block
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!("Connection {}", current_chat));

    let messages = app.server.get_messages(current_chat + 1);
    match messages {
        Some(messages) => {
            let messages_list_item: Vec<ListItem> = messages
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let content = vec![Spans::from(Span::raw(format!("{}: {}", i, m)))];
                    ListItem::new(content)
                })
                .skip(app.scroll)
                .collect();
            let messages_list = List::new(messages_list_item).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Connection {}", current_chat + 1)),
            );
            f.render_widget(messages_list, chunks[0]);
        }
        None => f.render_widget(block, area),
    }

    let input = Paragraph::new(app.input.as_ref())
        .style(match app.input_mode {
            InputMode::Normal => Style::default(),
            InputMode::Editing => Style::default().fg(Color::Yellow),
        })
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input, chunks[1]);

    match app.input_mode {
        InputMode::Normal =>
            // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
            {}

        InputMode::Editing => {
            // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
            f.set_cursor(
                // Put cursor past the end of the input text
                chunks[1].x + app.input.width() as u16 + 1,
                // Move one line down, from the border to the input line
                chunks[1].y + 1,
            )
        }
    }
}
