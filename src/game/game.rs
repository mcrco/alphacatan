use uuid::Uuid;

use crate::game::action::GameAction;
use crate::game::{GameConfig, GamePhase, GameState};
use crate::players::BasePlayer;
use crate::types::Color;

const TURNS_LIMIT: u32 = 1000;

pub struct Game {
    pub seed: u64,
    pub id: Uuid,
    pub vps_to_win: u8,
    pub state: GameState,
}

impl Game {
    pub fn new(config: GameConfig) -> Self {
        Self {
            seed: config.seed,
            id: Uuid::new_v4(),
            vps_to_win: config.vps_to_win,
            state: GameState::new(config),
        }
    }

    pub fn play<P: BasePlayer>(&mut self, players: &[P]) -> Option<Color> {
        while self.winning_color().is_none() && self.state.turn < TURNS_LIMIT {
            self.play_tick(players);
        }
        self.winning_color()
    }

    pub fn play_tick<P: BasePlayer>(&mut self, players: &[P]) -> Option<GameAction> {
        let current_idx = self.state.current_player;
        if current_idx >= players.len() {
            return None;
        }

        let legal_actions = self.state.legal_actions();
        if legal_actions.is_empty() {
            return None;
        }

        let player = &players[current_idx];
        let action = player.decide(self, legal_actions);

        if let Some(action) = action {
            self.execute(action.clone());
            Some(action)
        } else {
            None
        }
    }

    pub fn execute(&mut self, action: GameAction) {
        let _ = self.state.step(action);
    }

    pub fn winning_color(&self) -> Option<Color> {
        match &self.state.phase {
            GamePhase::Completed { winner } => {
                winner.and_then(|idx| self.state.players.get(idx).map(|p| p.color))
            }
            _ => {
                // Optimized: only check players that might have won recently
                // Check current player first (most likely to have just won)
                if let Some(player) = self.state.players.get(self.state.current_player) {
                    if player.total_points() >= self.vps_to_win {
                        return Some(player.color);
                    }
                }
                // Then check other players (but limit to avoid checking all every time)
                for (idx, player) in self.state.players.iter().enumerate() {
                    if idx != self.state.current_player && player.total_points() >= self.vps_to_win
                    {
                        return Some(player.color);
                    }
                }
                None
            }
        }
    }

    pub fn copy(&self) -> Self {
        Self {
            seed: self.seed,
            id: self.id,
            vps_to_win: self.vps_to_win,
            state: self.state.clone(),
        }
    }
}
