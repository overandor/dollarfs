use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Margin},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame, Terminal,
};
use std::io;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    TopFiles,
    SecurityFindings,
}

pub struct AppState {
    pub file_count: i64,
    pub total_value: f64,
    pub sec_count: i64,
    pub top_files: Vec<(String, f64, f64, String)>,
    pub security_findings: Vec<(String, i64, String, String, String)>,
    pub table_state: TableState,
    pub view_mode: ViewMode,
    pub should_quit: bool,
}

impl AppState {
    pub fn new(
        file_count: i64,
        total_value: f64,
        sec_count: i64,
        top_files: Vec<(String, f64, f64, String)>,
        security_findings: Vec<(String, i64, String, String, String)>,
    ) -> Self {
        let mut table_state = TableState::default();
        if !top_files.is_empty() {
            table_state.select(Some(0));
        }
        Self {
            file_count,
            total_value,
            sec_count,
            top_files,
            security_findings,
            table_state,
            view_mode: ViewMode::TopFiles,
            should_quit: false,
        }
    }

    pub fn on_tick(&mut self) {}

    fn row_count(&self) -> usize {
        match self.view_mode {
            ViewMode::TopFiles => self.top_files.len(),
            ViewMode::SecurityFindings => self.security_findings.len(),
        }
    }

    pub fn on_key(&mut self, key: crossterm::event::KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('s') => {
                self.view_mode = match self.view_mode {
                    ViewMode::TopFiles => ViewMode::SecurityFindings,
                    ViewMode::SecurityFindings => ViewMode::TopFiles,
                };
                self.table_state.select(Some(0));
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.table_state.selected().unwrap_or(0);
                let next = (i + 1).min(self.row_count().saturating_sub(1));
                self.table_state.select(Some(next));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self.table_state.selected().unwrap_or(0);
                let prev = i.saturating_sub(1);
                self.table_state.select(Some(prev));
            }
            _ => {}
        }
    }
}

pub fn run_tui(
    file_count: i64,
    total_value: f64,
    sec_count: i64,
    top_files: Vec<(String, f64, f64, String)>,
    security_findings: Vec<(String, i64, String, String, String)>,
) -> Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = AppState::new(file_count, total_value, sec_count, top_files, security_findings);

    let mut last_tick = std::time::Instant::now();
    let tick_rate = std::time::Duration::from_millis(250);

    loop {
        terminal.draw(|f| draw_ui(f, &mut app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                app.on_key(key);
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = std::time::Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn draw_ui(frame: &mut Frame, app: &mut AppState) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let header = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" lfv — local file value system ")
        .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    frame.render_widget(header, main_layout[0]);

    let stats_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(33), Constraint::Percentage(33), Constraint::Percentage(34)])
        .split(main_layout[0].inner(Margin::new(1, 1)));

    let stat_style = Style::default().fg(Color::White);
    let label_style = Style::default().fg(Color::Gray);

    let files_para = Paragraph::new(Text::from(vec![
        Line::from(Span::styled(format!("{}", app.file_count), stat_style.add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("files indexed", label_style)),
    ]));
    frame.render_widget(files_para, stats_layout[0]);

    let value_para = Paragraph::new(Text::from(vec![
        Line::from(Span::styled(format!("${:.0}", app.total_value), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("book value", label_style)),
    ]));
    frame.render_widget(value_para, stats_layout[1]);

    let sec_color = if app.sec_count > 0 { Color::Red } else { Color::Green };
    let sec_para = Paragraph::new(Text::from(vec![
        Line::from(Span::styled(format!("{}", app.sec_count), Style::default().fg(sec_color).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("security findings", label_style)),
    ]));
    frame.render_widget(sec_para, stats_layout[2]);

    match app.view_mode {
        ViewMode::TopFiles => draw_top_files_table(frame, main_layout[1], app),
        ViewMode::SecurityFindings => draw_security_table(frame, main_layout[1], app),
    }

    let footer = Paragraph::new(Text::from(vec![
        Line::from(Span::styled(
            " q quit | j/↓ next | k/↑ prev | s switch view ",
            Style::default().fg(Color::DarkGray),
        )),
    ]));
    frame.render_widget(footer, main_layout[2]);
}

fn draw_top_files_table(frame: &mut Frame, area: ratatui::layout::Rect, app: &mut AppState) {
    let table_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" top files ")
        .title_style(Style::default().fg(Color::White));

    let header_cells = ["Path", "Value", "Conf", "Reason"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(0);

    let rows = app.top_files.iter().map(|(path, value, conf, reason)| {
        let conf_label = if *conf >= 0.8 { "High" } else if *conf >= 0.5 { "Med" } else { "Low" };
        let conf_color = if *conf >= 0.8 { Color::Green } else if *conf >= 0.5 { Color::Yellow } else { Color::Red };
        let truncated_path = if path.len() > 50 { format!("{}...", &path[..47]) } else { path.clone() };
        let truncated_reason = if reason.len() > 40 { format!("{}...", &reason[..37]) } else { reason.clone() };
        Row::new(vec![
            Cell::from(truncated_path),
            Cell::from(format!("${:.0}", value)).style(Style::default().fg(Color::Cyan)),
            Cell::from(conf_label).style(Style::default().fg(conf_color)),
            Cell::from(truncated_reason).style(Style::default().fg(Color::Gray)),
        ])
    });

    let table = Table::new(rows, [
        Constraint::Percentage(45),
        Constraint::Length(12),
        Constraint::Length(6),
        Constraint::Percentage(40),
    ])
    .header(header)
    .block(table_block)
    .row_highlight_style(Style::default().bg(Color::Rgb(30, 30, 30)).add_modifier(Modifier::BOLD))
    .highlight_symbol(">> ");

    frame.render_stateful_widget(table, area, &mut app.table_state);
}

fn draw_security_table(frame: &mut Frame, area: ratatui::layout::Rect, app: &mut AppState) {
    let table_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" security findings ")
        .title_style(Style::default().fg(Color::Red));

    let header_cells = ["Path", "Line", "Type", "Severity", "Preview"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(0);

    let rows = app.security_findings.iter().map(|(path, line, finding_type, severity, preview)| {
        let sev_color = match severity.as_str() {
            "critical" => Color::Red,
            "high" => Color::Yellow,
            _ => Color::Cyan,
        };
        let truncated_path = if path.len() > 35 { format!("{}...", &path[..32]) } else { path.clone() };
        Row::new(vec![
            Cell::from(truncated_path),
            Cell::from(format!("{}", line)),
            Cell::from(finding_type.clone()).style(Style::default().fg(Color::Gray)),
            Cell::from(severity.clone()).style(Style::default().fg(sev_color)),
            Cell::from(preview.clone()).style(Style::default().fg(Color::DarkGray)),
        ])
    });

    let table = Table::new(rows, [
        Constraint::Percentage(35),
        Constraint::Length(6),
        Constraint::Percentage(25),
        Constraint::Length(10),
        Constraint::Percentage(25),
    ])
    .header(header)
    .block(table_block)
    .row_highlight_style(Style::default().bg(Color::Rgb(30, 30, 30)).add_modifier(Modifier::BOLD))
    .highlight_symbol(">> ");

    frame.render_stateful_widget(table, area, &mut app.table_state);
}
