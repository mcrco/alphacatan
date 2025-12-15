use std::collections::HashMap;
use std::io::{self, stdout, Stdout};
use std::process;
use std::time::Duration;

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::cli::compressed_actions::{action_detail_label, compress_actions, CompressedActionGroup, expand_group};
use crate::game::action::GameAction;
use crate::game::game::Game;
use crate::types::{ActionType, Color as PlayerColor};

type Terminal = ratatui::Terminal<CrosstermBackend<Stdout>>;

pub struct TuiApp {
    game: Game,
    human_color: PlayerColor,
    actions: Vec<GameAction>,
    compressed_groups: Vec<CompressedActionGroup>,
    selected_action_idx: usize,
    expanded_group: Option<usize>, // Group index if expanded
    expanded_map: HashMap<usize, usize>, // Maps expanded index -> original index
    show_help: bool,
    should_quit: bool,
    selected_action: Option<GameAction>,
}

impl TuiApp {
    pub fn new(game: Game, human_color: PlayerColor, actions: Vec<GameAction>) -> Self {
        let compressed_groups = compress_actions(&actions);
        let expanded_map = HashMap::new();
        
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
        let _ = execute!(
            terminal.backend_mut(),
            DisableMouseCapture
        );
        let _ = terminal.show_cursor();

        result
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                // User wants to quit the game entirely - exit the program
                // Cleanup terminal first
                let _ = disable_raw_mode();
                let _ = execute!(
                    io::stdout(),
                    DisableMouseCapture
                );
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
                Constraint::Min(20), // Main content
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
        let board_text = self.generate_board_text();
        
        let lines: Vec<Line<'_>> = board_text
            .lines()
            .map(|line| {
                Line::from(
                    line.chars()
                        .map(|c| {
                            let style = self.color_for_board_char(c);
                            Span::styled(c.to_string(), style)
                        })
                        .collect::<Vec<_>>()
                )
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Board")
            .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

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
                Constraint::Percentage(50), // Game state
                Constraint::Percentage(50), // Actions
            ])
            .split(area);

        self.render_game_state(f, chunks[0]);
        self.render_actions(f, chunks[1]);
    }

    fn render_game_state(&self, f: &mut Frame<'_>, area: Rect) {
        let human_idx = self.game.state.players.iter()
            .position(|p| p.color == self.human_color)
            .unwrap_or(0);

        let mut lines: Vec<Line<'_>> = vec![];
        lines.push(Line::from(vec![
            Span::styled("Turn ", Style::default()),
            Span::styled(format!("{}", self.game.state.turn), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
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
                Span::styled(format!("{} ({:?})", label, player.color), Style::default().fg(color).add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::from(vec![
                Span::raw("  VP: "),
                Span::styled(format!("{}", player.total_points()), Style::default().fg(Color::Green)),
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
                    format!("{}S {}C {}R", 
                        player.settlements.len(),
                        player.cities.len(),
                        player.roads.len()),
                    Style::default()
                ),
            ]));
        }

        // Last roll
        if let Some((d1, d2)) = self.game.state.last_roll {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::raw("Last Roll: "),
                Span::styled(format!("{} + {} = {}", d1, d2, d1 + d2), Style::default().fg(Color::Yellow)),
            ]));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Game State");

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });

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
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                
                items.push(ListItem::new(format!("  {}", details)).style(style));
            }
        } else {
            // Show compressed groups
            for (idx, group) in self.compressed_groups.iter().enumerate() {
                let style = if idx == self.selected_action_idx {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
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
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

        let mut state = ListState::default();
        state.select(Some(self.selected_action_idx));

        f.render_stateful_widget(list, area, &mut state);
    }

    fn render_status_bar(&self, f: &mut Frame<'_>, area: Rect) {
        let help_text = if self.show_help {
            "↑/↓: Navigate | Enter: Select/Expand | ←/Backspace: Back | h: Toggle Help | q/Esc: Quit"
        } else {
            "Press 'h' for help | q/Esc to quit"
        };

        let paragraph = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center);

        f.render_widget(paragraph, area);
    }

    fn generate_board_text(&self) -> String {
        crate::cli::board_display::render_board_to_string(&self.game)
    }
    fn color_for_board_char(&self, c: char) -> Style {
        match c {
            'W' => Style::default().fg(Color::Green), // Wood
            'B' => Style::default().fg(Color::Red), // Brick
            'S' => Style::default().fg(Color::White), // Sheep
            'H' => Style::default().fg(Color::Yellow), // Wheat
            'O' => Style::default().fg(Color::Magenta), // Ore
            'D' => Style::default().fg(Color::DarkGray), // Desert
            'r' => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD), // Red settlement
            'R' => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD), // Red city
            'b' => Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD), // Blue settlement
            'o' => Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD), // Orange settlement
            'w' => Style::default().fg(Color::White).add_modifier(Modifier::BOLD), // White settlement
            '🔴' => Style::default().fg(Color::Red), // Robber
            _ => Style::default(),
        }
    }

    fn color_for_player(&self, color: PlayerColor) -> Color {
        match color {
            PlayerColor::Red => Color::Red,
            PlayerColor::Blue => Color::Blue,
            PlayerColor::Orange => Color::Magenta,
            PlayerColor::White => Color::White,
        }
    }
}

