use crate::cli::tui::TuiApp;
use crate::game::action::GameAction;
use crate::game::game::Game;
use crate::players::BasePlayer;
use crate::types::Color;

#[derive(Clone)]
pub struct HumanPlayer {
    pub color: Color,
}

impl HumanPlayer {
    pub fn new(color: Color) -> Self {
        Self { color }
    }
}

impl BasePlayer for HumanPlayer {
    fn decide(&self, game: &Game, actions: &[GameAction]) -> Option<GameAction> {
        if actions.is_empty() {
            return None;
        }

        // Use TUI for beautiful interactive interface
        let mut app = TuiApp::new(game.copy(), self.color, actions.to_vec());
        match app.run() {
            Ok(action) => action,
            Err(_) => None,
        }
    }
}

