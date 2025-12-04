use crate::game::action::GameAction;
use crate::game::game::Game;
use crate::players::{
    BasePlayer, MCTSPlayer, RandomPlayer, ValueFunctionPlayer, ValueFunctionParams,
};
use crate::types::Color;

pub struct CliPlayer {
    pub code: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

pub const CLI_PLAYERS: &[CliPlayer] = &[
    CliPlayer {
        code: "R",
        name: "RandomPlayer",
        description: "Chooses actions at random.",
    },
    CliPlayer {
        code: "F",
        name: "ValueFunctionPlayer",
        description: "Chooses the action that leads to the most immediate reward, based on a hand-crafted value function.",
    },
    CliPlayer {
        code: "M",
        name: "MCTSPlayer",
        description: "Decides according to the MCTS algorithm. First param is NUM_SIMULATIONS.",
    },
];

#[derive(Clone)]
pub enum PlayerInstance {
    Random(RandomPlayer),
    ValueFunction(ValueFunctionPlayer),
    MCTS(MCTSPlayer),
}

impl BasePlayer for PlayerInstance {
    fn decide(&self, game: &Game, actions: &[GameAction]) -> Option<GameAction> {
        match self {
            PlayerInstance::Random(p) => p.decide(game, actions),
            PlayerInstance::ValueFunction(p) => p.decide(game, actions),
            PlayerInstance::MCTS(p) => p.decide(game, actions),
        }
    }
}

pub fn create_player(code: &str, color: Color, params: Vec<&str>) -> Option<PlayerInstance> {
    match code {
        "R" => Some(PlayerInstance::Random(RandomPlayer)),
        "F" => {
            let value_params = if params.is_empty() {
                ValueFunctionParams::default()
            } else {
                // For now, use default params. Could parse custom params later
                ValueFunctionParams::default()
            };
            Some(PlayerInstance::ValueFunction(ValueFunctionPlayer::new(
                color,
                Some(value_params),
                None,
            )))
        }
        "M" => {
            // First param: number of simulations, default SIMULATIONS
            let num_sims = params
                .get(0)
                .and_then(|s| s.parse::<usize>().ok());
            // Second param (optional): prunning flag (any value other than explicit "false" is treated as true)
            let prunning = params.get(1).map(|s| s.to_lowercase() != "false");
            Some(PlayerInstance::MCTS(MCTSPlayer::new(
                color,
                num_sims,
                prunning,
            )))
        }
        _ => None,
    }
}

pub fn print_player_help() {
    println!("Player Legend:");
    println!("{:<5} {:<25} {}", "CODE", "PLAYER", "DESCRIPTION");
    println!("{}", "-".repeat(80));
    for player in CLI_PLAYERS {
        println!("{:<5} {:<25} {}", player.code, player.name, player.description);
    }
}

