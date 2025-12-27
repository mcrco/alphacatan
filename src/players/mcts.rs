use std::collections::HashMap;

use rand::seq::SliceRandom;

use crate::game::action::GameAction;
use crate::game::game::Game;
use crate::players::BasePlayer;
use crate::players::tree_search::{execute_spectrum, list_pruned_actions};
use crate::types::Color;

const SIMULATIONS: usize = 10;
const EPSILON: f64 = 1e-8;

fn exp_c() -> f64 {
    2.0_f64.sqrt()
}

#[derive(Clone)]
pub struct MCTSPlayer {
    pub color: Color,
    pub num_simulations: usize,
    pub prunning: bool,
}

impl MCTSPlayer {
    pub fn new(color: Color, num_simulations: Option<usize>, prunning: Option<bool>) -> Self {
        Self {
            color,
            num_simulations: num_simulations.unwrap_or(SIMULATIONS),
            prunning: prunning.unwrap_or(false),
        }
    }
}

impl BasePlayer for MCTSPlayer {
    fn decide(&self, game: &Game, _actions: &[GameAction]) -> Option<GameAction> {
        // Mirror Python: choose between raw playable_actions or pruned ones
        let base_actions: Vec<GameAction> = game.state.legal_actions().to_vec();
        let actions = if self.prunning {
            list_pruned_actions(game)
        } else {
            base_actions
        };

        if actions.len() <= 1 {
            return actions.first().cloned();
        }

        let mut root = StateNode::new(self.color, game.copy(), self.prunning);
        for _ in 0..self.num_simulations {
            root.run_simulation();
        }

        root.choose_best_action(&actions)
    }
}

struct StateNode {
    level: usize,
    color: Color,
    game: Game,
    children: HashMap<GameAction, Vec<(Box<StateNode>, f64)>>,
    prunning: bool,
    wins: u32,
    visits: u32,
}

impl StateNode {
    fn new(color: Color, game: Game, prunning: bool) -> Self {
        Self {
            level: 0,
            color,
            game,
            children: HashMap::new(),
            prunning,
            wins: 0,
            visits: 0,
        }
    }

    fn run_simulation(&mut self) {
        // Simplified mirror of Python MCTS:
        // If leaf and non-terminal, expand once; then playout from this node.
        if self.is_leaf() && !self.is_terminal() {
            self.expand();
        }

        // Select best action and run playout
        let action = self.choose_best_action_for_selection();
        let result = self.playout();

        // Update statistics
        self.visits += 1;
        if result == Some(self.color) {
            self.wins += 1;
        }

        // Update children if they exist
        if let Some(children) = self.children.get_mut(&action) {
            for (child, _) in children.iter_mut() {
                child.visits += 1;
                if result == Some(self.color) {
                    child.wins += 1;
                }
            }
        }
    }

    fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn is_terminal(&self) -> bool {
        self.game.winning_color().is_some()
    }

    fn expand(&mut self) {
        // Use the same pruning rule as the Python list_prunned_actions when enabled
        let base = self.game.state.legal_actions().to_vec();
        let actions = if self.prunning {
            list_pruned_actions(&self.game)
        } else {
            base
        };

        for action in actions {
            let outcomes = execute_spectrum(&self.game, &action);
            for (next_game, p) in outcomes {
                let child = StateNode::new(self.color, next_game, self.prunning);
                self.children
                    .entry(action.clone())
                    .or_insert_with(Vec::new)
                    .push((Box::new(child), p));
            }
        }
    }

    fn choose_best_action(&self, actions: &[GameAction]) -> Option<GameAction> {
        let mut best_action = None;
        let mut best_score = f64::NEG_INFINITY;

        for action in actions {
            let score = self.action_children_expected_score(action);
            if score > best_score {
                best_score = score;
                best_action = Some(action.clone());
            }
        }

        best_action
    }

    fn choose_best_action_for_selection(&self) -> GameAction {
        // When children exist, base the choice on them; otherwise fall back to legal actions.
        if !self.children.is_empty() {
            let mut best_action: Option<GameAction> = None;
            let mut best_score = f64::NEG_INFINITY;
            for (action, _) in &self.children {
                let score = self.action_children_expected_score(action);
                if score > best_score {
                    best_score = score;
                    best_action = Some(action.clone());
                }
            }
            if let Some(a) = best_action {
                return a;
            }
        }

        let actions: Vec<_> = self.game.state.legal_actions().to_vec();
        if actions.is_empty() {
            return GameAction::new(
                self.game.state.current_player,
                crate::types::ActionType::EndTurn,
            );
        }
        actions[0].clone()
    }

    fn action_children_expected_score(&self, action: &GameAction) -> f64 {
        if let Some(children) = self.children.get(action) {
            let mut score = 0.0;
            for (child, proba) in children {
                let win_rate = if child.visits > 0 {
                    child.wins as f64 / child.visits as f64
                } else {
                    0.0
                };
                let ucb = exp_c()
                    * ((self.visits as f64 + EPSILON).ln() / (child.visits as f64 + EPSILON))
                        .sqrt();
                score += proba * (win_rate + ucb);
            }
            score
        } else {
            // Unexplored action - use UCB1 with 0 visits
            exp_c() * ((self.visits as f64 + EPSILON).ln() / EPSILON).sqrt()
        }
    }

    fn playout(&mut self) -> Option<Color> {
        // Run a random playout to completion
        let mut game_copy = self.game.copy();
        let mut rng = rand::thread_rng();

        // Use RandomPlayer logic for playout
        while game_copy.winning_color().is_none() && game_copy.state.turn < 1000 {
            let legal_actions = game_copy.state.legal_actions();
            if legal_actions.is_empty() {
                break;
            }

            if let Some(action) = legal_actions.choose(&mut rng) {
                game_copy.execute(action.clone());
            } else {
                break;
            }
        }

        game_copy.winning_color()
    }
}
