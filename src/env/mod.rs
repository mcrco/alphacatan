use serde::{Deserialize, Serialize};

use crate::features::{BoardTensor, FeatureCollection, build_board_tensor, collect_features};
use crate::game::{GameConfig, GameError, GameEvent, GameState, action::GameAction};
use crate::types::{ActionPrompt, Color, Resource};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerObservation {
    pub color: Color,
    pub resources: [u8; Resource::ALL.len()],
    pub dev_cards: usize,
    pub fresh_dev_cards: usize,
    pub settlements: usize,
    pub cities: usize,
    pub roads: usize,
    pub victory_points: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub current_player: usize,
    pub pending_prompt: ActionPrompt,
    pub turn: u32,
    pub last_roll: Option<(u8, u8)>,
    pub players: Vec<PlayerObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub observation: Observation,
    pub rewards: Vec<f32>,
    pub done: bool,
    pub events: Vec<GameEvent>,
}

#[derive(Debug, Clone)]
pub struct RustEnv {
    state: GameState,
}

impl RustEnv {
    pub fn new(config: GameConfig) -> Self {
        Self {
            state: GameState::new(config),
        }
    }

    pub fn reset(&mut self) -> Observation {
        self.state.reset();
        observation_from_state(&self.state)
    }

    pub fn step(&mut self, action: GameAction) -> Result<StepResult, GameError> {
        let outcome = self.state.step(action)?;
        Ok(StepResult {
            observation: observation_from_state(&self.state),
            rewards: outcome.rewards,
            done: outcome.done,
            events: outcome.events,
        })
    }

    pub fn pending_prompt(&self) -> ActionPrompt {
        self.state.legal_action_prompt()
    }

    pub fn current_player(&self) -> usize {
        self.state.current_player
    }

    pub fn game_state(&self) -> &GameState {
        &self.state
    }

    pub fn game_state_mut(&mut self) -> &mut GameState {
        &mut self.state
    }

    pub fn extract_features(
        &self,
        player_index: usize,
    ) -> Option<(FeatureCollection, BoardTensor)> {
        if player_index >= self.state.players.len() {
            return None;
        }
        let numeric = collect_features(&self.state, player_index);
        let tensor = build_board_tensor(&self.state, player_index);
        Some((numeric, tensor))
    }
}

pub fn observation_from_state(state: &GameState) -> Observation {
    Observation {
        current_player: state.current_player,
        pending_prompt: state.legal_action_prompt(),
        turn: state.turn,
        last_roll: state.last_roll,
        players: state
            .players
            .iter()
            .map(|player| PlayerObservation {
                color: player.color,
                resources: player.resources.counts(),
                dev_cards: player.dev_cards.len(),
                fresh_dev_cards: player.fresh_dev_cards.len(),
                settlements: player.settlements.len(),
                cities: player.cities.len(),
                roads: player.roads.len(),
                victory_points: player.total_points(),
            })
            .collect(),
    }
}
