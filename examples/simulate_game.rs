use catanatron_rs::game::{Game, GameConfig};
use catanatron_rs::players::{MCTSPlayer, RandomPlayer, ValueFunctionPlayer, ValueFunctionParams};
use catanatron_rs::{Color, MapType};

fn main() {
    // Create game configuration
    let config = GameConfig {
        num_players: 2,
        map_type: MapType::Base,
        vps_to_win: 10,
        seed: 42,
    };

    // Example 1: Two RandomPlayers
    println!("Example 1: RandomPlayer vs RandomPlayer");
    let random_players = [RandomPlayer, RandomPlayer];
    let mut game1 = Game::new(config.clone());
    let winner1 = game1.play(&random_players);
    match winner1 {
        Some(color) => println!("  Winner: {:?} in {} turns", color, game1.state.turn),
        None => println!("  Game exceeded turn limit at {} turns", game1.state.turn),
    }

    // Example 2: Two ValueFunctionPlayers
    println!("\nExample 2: ValueFunctionPlayer vs ValueFunctionPlayer");
    let value_params = ValueFunctionParams::default();
    let value_players = [
        ValueFunctionPlayer::new(Color::Red, Some(value_params.clone()), None),
        ValueFunctionPlayer::new(Color::Blue, Some(value_params), None),
    ];
    let mut game2 = Game::new(config.clone());
    let winner2 = game2.play(&value_players);
    match winner2 {
        Some(color) => println!("  Winner: {:?} in {} turns", color, game2.state.turn),
        None => println!("  Game exceeded turn limit at {} turns", game2.state.turn),
    }

    // Example 3: MCTSPlayer vs MCTSPlayer
    println!("\nExample 3: MCTSPlayer vs MCTSPlayer");
    let mcts_players = [
        MCTSPlayer::new(Color::Red, Some(10), None),
        MCTSPlayer::new(Color::Blue, Some(10), None),
    ];
    let mut game3 = Game::new(config);
    let winner3 = game3.play(&mcts_players);
    match winner3 {
        Some(color) => println!("  Winner: {:?} in {} turns", color, game3.state.turn),
        None => println!("  Game exceeded turn limit at {} turns", game3.state.turn),
    }
}

