use std::collections::HashMap;
use std::io::{self, Stdout, stdout};
use std::process;
use std::time::Duration;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::Frame;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::board::NodeId;
use crate::cli::board_display::{NodeSpan, render_board as render_ascii_board};
use crate::cli::compressed_actions::{
    CompressedActionGroup, action_detail_label, compress_actions, expand_group,
};
use crate::game::action::GameAction;
use crate::game::game::Game;
use crate::game::state::Structure;
use crate::types::{Color as PlayerColor, DevelopmentCard};

pub type Terminal = ratatui::Terminal<CrosstermBackend<Stdout>>;

pub struct TuiApp {
    game: Game,
    human_color: PlayerColor,
    actions: Vec<GameAction>,
    compressed_groups: Vec<CompressedActionGroup>,
    selected_action_idx: usize,
    expanded_group: Option<usize>,       // Group index if expanded
    expanded_map: HashMap<usize, usize>, // Maps expanded index -> original index
    show_help: bool,
    should_quit: bool,
    selected_action: Option<GameAction>,
    history: Vec<GameAction>,
    game_state_scroll: u16,
    history_scroll: u16,
    game_state_max_scroll: u16,
    history_max_scroll: u16,
}

impl TuiApp {
    pub fn new(game: Game, human_color: PlayerColor, actions: Vec<GameAction>) -> Self {
        let compressed_groups = compress_actions(&actions);
        let expanded_map = HashMap::new();

        let history = game.state.actions.clone();

        Self {
            game,
            human_color,
            actions,
            compressed_groups,
            selected_action_idx: 0,
            expanded_group: None,
            expanded_map,
            show_help: false,
            should_quit: false,
            selected_action: None,
            history,
            game_state_scroll: 0,
            history_scroll: 0,
            game_state_max_scroll: 0,
            history_max_scroll: 0,
        }
    }

    pub fn run(&mut self) -> io::Result<Option<GameAction>> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?; // clear cargo/run output before first draw

        let result = loop {
            if self.should_quit {
                break Ok(self.selected_action.take());
            }

            terminal.draw(|f| self.render(f))?;

            if crossterm::event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        if self.handle_key(key) {
                            // handle_key returned true, meaning we should quit or action was selected
                            break Ok(self.selected_action.take());
                        }
                    }
                }
            }
        };

        // Always cleanup terminal state
        let _ = terminal.clear();
        let _ = disable_raw_mode();
        let _ = execute!(terminal.backend_mut(), DisableMouseCapture);
        let _ = terminal.show_cursor();

        result
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            let is_shift = key.modifiers.contains(KeyModifiers::SHIFT);
            match key.code {
                KeyCode::Up => {
                    if is_shift {
                        self.adjust_history_scroll(-1);
                    } else {
                        self.adjust_game_state_scroll(-1);
                    }
                    return false;
                }
                KeyCode::Down => {
                    if is_shift {
                        self.adjust_history_scroll(1);
                    } else {
                        self.adjust_game_state_scroll(1);
                    }
                    return false;
                }
                KeyCode::PageUp => {
                    if is_shift {
                        self.adjust_history_scroll(-5);
                    } else {
                        self.adjust_game_state_scroll(-5);
                    }
                    return false;
                }
                KeyCode::PageDown => {
                    if is_shift {
                        self.adjust_history_scroll(5);
                    } else {
                        self.adjust_game_state_scroll(5);
                    }
                    return false;
                }
                _ => {}
            }
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                // User wants to quit the game entirely - exit the program
                // Cleanup terminal first
                let _ = disable_raw_mode();
                let _ = execute!(io::stdout(), DisableMouseCapture);
                process::exit(0);
            }
            KeyCode::Char('h') => {
                self.show_help = !self.show_help;
            }
            KeyCode::Up => {
                if self.selected_action_idx > 0 {
                    self.selected_action_idx -= 1;
                }
            }
            KeyCode::Down => {
                let max_idx = if self.expanded_group.is_some() {
                    self.expanded_map.len()
                } else {
                    self.compressed_groups.len()
                };
                if self.selected_action_idx < max_idx.saturating_sub(1) {
                    self.selected_action_idx += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(_expanded_idx) = self.expanded_group {
                    // In expanded mode - select from expanded actions
                    if let Some(&original_idx) = self.expanded_map.get(&self.selected_action_idx) {
                        if original_idx < self.actions.len() {
                            self.selected_action = Some(self.actions[original_idx].clone());
                            return true;
                        }
                    }
                } else {
                    // Normal mode - check if it's a group or single action
                    if self.selected_action_idx < self.compressed_groups.len() {
                        let group = &self.compressed_groups[self.selected_action_idx];
                        if group.actions.len() == 1 {
                            // Single action - select it
                            let (original_idx, _) = &group.actions[0];
                            self.selected_action = Some(self.actions[*original_idx].clone());
                            return true;
                        } else {
                            // Expand the group
                            self.expanded_group = Some(self.selected_action_idx);
                            self.expanded_map = expand_group(group, 0);
                            self.selected_action_idx = 0;
                        }
                    }
                }
            }
            KeyCode::Backspace | KeyCode::Left => {
                // Go back from expanded view
                if self.expanded_group.is_some() {
                    self.expanded_group = None;
                    self.expanded_map.clear();
                    self.selected_action_idx = 0;
                }
            }
            _ => {}
        }
        false
    }

    fn render(&mut self, f: &mut Frame<'_>) {
        let area = f.size();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(20),   // Main content
                Constraint::Length(3), // Help/status bar
            ])
            .split(area);

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60), // Board
                Constraint::Percentage(40), // Game info and actions
            ])
            .split(chunks[0]);

        // Render board on left
        self.render_board(f, main_chunks[0]);

        // Render game info and actions on right
        self.render_right_panel(f, main_chunks[1]);

        // Render status/help bar at bottom
        self.render_status_bar(f, chunks[1]);
    }

    fn render_board(&self, f: &mut Frame<'_>, area: Rect) {
        let rendered_board = render_ascii_board(&self.game);
        let mut span_lookup: HashMap<(usize, usize), (NodeId, NodeSpan)> = HashMap::new();
        for (node_id, span) in &rendered_board.node_spans {
            span_lookup.insert((span.row, span.col_start), (*node_id, *span));
        }

        let lines: Vec<Line<'_>> = rendered_board
            .text
            .lines()
            .enumerate()
            .map(|(row_idx, line)| {
                let chars: Vec<char> = line.chars().collect();
                let mut spans: Vec<Span<'_>> = Vec::new();
                let mut col = 0;
                while col < chars.len() {
                    if let Some((node_id, span)) = span_lookup.get(&(row_idx, col)) {
                        if let Some(style) = self.style_for_node(*node_id) {
                            let segment: String = chars[col..col + span.len].iter().collect();
                            spans.push(Span::styled(segment, style));
                            col += span.len;
                            continue;
                        }
                    }

                    let ch = chars[col];
                    let style = self.color_for_board_char(ch);
                    spans.push(Span::styled(ch.to_string(), style));
                    col += 1;
                }
                Line::from(spans)
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Board")
            .title_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );

        let paragraph = Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, area);
    }

    fn render_right_panel(&mut self, f: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(35), // Game state
                Constraint::Percentage(35), // Actions
                Constraint::Percentage(30), // History
            ])
            .split(area);

        self.render_game_state(f, chunks[0]);
        self.render_actions(f, chunks[1]);
        self.render_history_panel(f, chunks[2]);
    }

    fn render_game_state(&mut self, f: &mut Frame<'_>, area: Rect) {
        let human_idx = self
            .game
            .state
            .players
            .iter()
            .position(|p| p.color == self.human_color)
            .unwrap_or(0);

        let mut lines: Vec<Line<'_>> = vec![];
        lines.push(Line::from(vec![
            Span::styled("Turn ", Style::default()),
            Span::styled(
                format!("{}", self.game.state.turn),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        // Player info
        for (idx, player) in self.game.state.players.iter().enumerate() {
            let is_current = idx == self.game.state.current_player;
            let is_human = idx == human_idx;
            let color = self.color_for_player(player.color);
            let marker = if is_current { "→ " } else { "  " };
            let label = if is_human { "YOU" } else { "BOT" };

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{} ({:?})", label, player.color),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  VP: "),
                Span::styled(
                    format!("{}", player.total_points()),
                    Style::default().fg(Color::Green),
                ),
            ]));

            // Resources
            let resources = format!("{}", player.resources);
            lines.push(Line::from(vec![
                Span::raw("  Resources: "),
                Span::styled(resources, Style::default()),
            ]));

            // Buildings
            lines.push(Line::from(vec![
                Span::raw("  Buildings: "),
                Span::styled(
                    format!(
                        "{}S {}C {}R",
                        player.settlements.len(),
                        player.cities.len(),
                        player.roads.len()
                    ),
                    Style::default(),
                ),
            ]));

            // Development cards
            lines.push(Line::from(vec![
                Span::raw("  Development cards: "),
                Span::styled(
                    format!(
                        "{}K {}Y {}M {}R {}V",
                        player.matured_dev_card_count(DevelopmentCard::Knight)
                            + player.fresh_dev_card_count(DevelopmentCard::Knight),
                        player.matured_dev_card_count(DevelopmentCard::YearOfPlenty)
                            + player.fresh_dev_card_count(DevelopmentCard::YearOfPlenty),
                        player.matured_dev_card_count(DevelopmentCard::Monopoly)
                            + player.fresh_dev_card_count(DevelopmentCard::Monopoly),
                        player.matured_dev_card_count(DevelopmentCard::RoadBuilding)
                            + player.fresh_dev_card_count(DevelopmentCard::RoadBuilding),
                        player.matured_dev_card_count(DevelopmentCard::VictoryPoint)
                            + player.fresh_dev_card_count(DevelopmentCard::VictoryPoint),
                    ),
                    Style::default(),
                ),
            ]));
        }

        // Last roll
        if let Some((d1, d2)) = self.game.state.last_roll {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::raw("Last Roll: "),
                Span::styled(
                    format!("{} + {} = {}", d1, d2, d1 + d2),
                    Style::default().fg(Color::Yellow),
                ),
            ]));
        }

        let block = Block::default().borders(Borders::ALL).title("Game State");

        let viewport_height = area.height.saturating_sub(2);
        let content_height = lines.len() as u16;
        let max_scroll = content_height.saturating_sub(viewport_height);
        self.game_state_max_scroll = max_scroll;
        if self.game_state_scroll > max_scroll {
            self.game_state_scroll = max_scroll;
        }

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.game_state_scroll, 0));

        f.render_widget(paragraph, area);
    }

    fn render_actions(&mut self, f: &mut Frame<'_>, area: Rect) {
        let mut items: Vec<ListItem<'_>> = vec![];

        if let Some(expanded_idx) = self.expanded_group {
            // Show expanded actions
            let group = &self.compressed_groups[expanded_idx];
            for (exp_idx, (_original_idx, action)) in group.actions.iter().enumerate() {
                let details = action_detail_label(action);
                let style = if exp_idx == self.selected_action_idx {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                items.push(ListItem::new(format!("  {}", details)).style(style));
            }
        } else {
            // Show compressed groups
            for (idx, group) in self.compressed_groups.iter().enumerate() {
                let style = if idx == self.selected_action_idx {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let text = if group.actions.len() == 1 {
                    group.description.clone()
                } else {
                    format!("{} ({} options)", group.description, group.actions.len())
                };

                items.push(ListItem::new(format!("[{}] {}", idx, text)).style(style));
            }
        }

        let title = if self.expanded_group.is_some() {
            "Available Actions (Expanded)"
        } else {
            "Available Actions"
        };

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );

        let mut state = ListState::default();
        state.select(Some(self.selected_action_idx));

        f.render_stateful_widget(list, area, &mut state);
    }

    fn render_history_panel(&mut self, f: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Action History");

        let mut lines: Vec<Line<'_>> = Vec::new();
        if self.history.is_empty() {
            lines.push(Line::from("No actions have been taken yet."));
        } else {
            for (idx, action) in self.history.iter().enumerate() {
                lines.push(Line::from(self.format_history_entry(idx, action)));
            }
        }

        let viewport_height = area.height.saturating_sub(2);
        let content_height = lines.len() as u16;
        let max_scroll = content_height.saturating_sub(viewport_height);
        self.history_max_scroll = max_scroll;
        self.ensure_history_scroll_from_bottom(viewport_height, content_height);

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.history_scroll, 0));

        f.render_widget(paragraph, area);
    }

    fn render_status_bar(&self, f: &mut Frame<'_>, area: Rect) {
        let help_text = if self.show_help {
            "↑/↓: Navigate | Enter: Select/Expand | ←/Backspace: Back | Ctrl+↑/↓: Scroll Game | Ctrl+Shift+↑/↓: Scroll History | h: Toggle Help | q/Esc: Quit"
        } else {
            "Press 'h' for help | Ctrl+↑/↓ game scroll | Ctrl+Shift+↑/↓ history scroll"
        };

        let paragraph = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center);

        f.render_widget(paragraph, area);
    }

    fn color_for_board_char(&self, c: char) -> Style {
        match c {
            'W' => Style::default().fg(Color::Green),    // Wood
            'B' => Style::default().fg(Color::LightRed), // Brick
            'S' => Style::default().fg(Color::White),    // Sheep
            'H' => Style::default().fg(Color::Yellow),   // Wheat
            'O' => Style::default().fg(Color::Magenta),  // Ore
            'D' => Style::default().fg(Color::DarkGray), // Desert
            'r' => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD), // Red settlement
            'R' => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD), // Red city
            'b' => Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD), // Blue settlement
            'o' => Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD), // Orange settlement
            'w' => Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD), // White settlement
            '🔴' => Style::default().fg(Color::Red),     // Robber
            _ => Style::default(),
        }
    }

    fn adjust_game_state_scroll(&mut self, delta: i16) {
        Self::adjust_scroll(
            &mut self.game_state_scroll,
            self.game_state_max_scroll,
            delta,
        );
    }

    fn adjust_history_scroll(&mut self, delta: i16) {
        Self::adjust_scroll(&mut self.history_scroll, self.history_max_scroll, delta);
    }

    fn adjust_scroll(current: &mut u16, max_scroll: u16, delta: i16) {
        let current_val = *current as i32 + delta as i32;
        let clamped = current_val.clamp(0, max_scroll as i32);
        *current = clamped as u16;
    }

    fn style_for_node(&self, node_id: NodeId) -> Option<Style> {
        let (player_idx, is_city) = match self.game.state.node_occupancy.get(&node_id)? {
            Structure::Settlement { player } => (*player, false),
            Structure::City { player } => (*player, true),
        };

        let player = self.game.state.players.get(player_idx)?;
        let mut style = Style::default().fg(self.color_for_player(player.color));
        if is_city {
            style = style.add_modifier(Modifier::BOLD);
        }
        Some(style)
    }

    fn color_for_player(&self, color: PlayerColor) -> Color {
        match color {
            PlayerColor::Red => Color::Red,
            PlayerColor::Blue => Color::Blue,
            PlayerColor::Orange => Color::Magenta,
            PlayerColor::White => Color::White,
        }
    }

    fn format_history_entry(&self, idx: usize, action: &GameAction) -> String {
        let player_label = self
            .game
            .state
            .players
            .get(action.player_index)
            .map(|player| {
                if player.color == self.human_color {
                    format!("YOU ({:?})", player.color)
                } else {
                    format!("{:?}", player.color)
                }
            })
            .unwrap_or_else(|| format!("Player {}", action.player_index));

        let action_type = format!("{:?}", action.action_type);
        let detail = action_detail_label(action);

        if detail == action_type {
            format!("#{} {} {}", idx + 1, player_label, action_type)
        } else {
            format!("#{} {} {} – {}", idx + 1, player_label, action_type, detail)
        }
    }

    fn ensure_history_scroll_from_bottom(&mut self, viewport_height: u16, content_height: u16) {
        if content_height <= viewport_height {
            self.history_scroll = 0;
        } else if self.history_scroll == 0 || self.history_scroll == self.history_max_scroll {
            self.history_scroll = self.history_max_scroll;
        } else if self.history_scroll > self.history_max_scroll {
            self.history_scroll = self.history_max_scroll;
        }
    }
}
