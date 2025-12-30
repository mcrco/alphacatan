use std::str::FromStr;
use std::time::Instant;

use catanatron_rs::MapType;
use catanatron_rs::game::{Game, GameConfig};
use catanatron_rs::players::RandomPlayer;

use clap::Parser;

#[derive(Debug, Parser, Clone)]
#[command(name = "settle-rs-profile-game")]
#[command(about = "Profile the game")]
struct Args {
    #[arg(long, default_value = "100000")]
    num_steps: u32,

    #[arg(long, default_value = "1000")]
    num_games: u32,

    /// Map type: BASE, MINI, or TOURNAMENT
    #[arg(long, default_value = "BASE")]
    map: String,

    #[arg(long, default_value = "2")]
    num_players: usize,

    #[arg(long, default_value = "1000")]
    turns_limit: u32,

    #[arg(long, default_value = "42")]
    seed: u64,
}

fn profile_steps(map: String, num_players: usize, num_steps: u32, turns_limit: u32, seed: u64) {
    let map_type = MapType::from_str(&map.to_uppercase()).unwrap_or_else(|_| {
        eprintln!("Error: Invalid map type '{}'. Use BASE, MINI, or TOURNAMENT", map);
        std::process::exit(1);
    });

    let mut config = GameConfig {
        num_players,
        map_type,
        vps_to_win: 10,
        seed,
    };

    let mut game = Game::new(config.clone());
    let mut players = Vec::new();
    for _i in 0..num_players {
        players.push(RandomPlayer);
    }

    let mut durations = Vec::new();
    for _ in 0..num_steps {
        if game.winning_color().is_some() || game.state.turn >= turns_limit {
            config.seed += 1;
            game = Game::new(config.clone());
        }
        let start = Instant::now();
        let _ = game.play_tick(&players);
        durations.push(start.elapsed());
    }

    if !durations.is_empty() {
        let total: u128 = durations.iter().map(|d| d.as_nanos()).sum();
        let avg_nanos = total / durations.len() as u128;
        let avg = std::time::Duration::from_nanos(avg_nanos as u64);

        let min = durations.iter().min().unwrap();
        let max = durations.iter().max().unwrap();

        println!("play_tick timing statistics:");
        println!("  Steps: {}", durations.len());
        println!("  Average: {:?}", avg);
        println!("  Min: {:?}", min);
        println!("  Max: {:?}", max);
    }
}

fn profile_games(map: String, num_players: usize, num_games: u32, seed: u64) {
    let map_type = MapType::from_str(&map.to_uppercase()).unwrap_or_else(|_| {
        eprintln!("Error: Invalid map type '{}'. Use BASE, MINI, or TOURNAMENT", map);
        std::process::exit(1);
    });

    let config = GameConfig {
        num_players,
        map_type,
        vps_to_win: 10,
        seed,
    };

    let mut players = Vec::new();
    for _i in 0..num_players {
        players.push(RandomPlayer);
    }

    let mut durations = Vec::new();
    let mut turns = Vec::new();
    for _ in 0..num_games {
        let start = Instant::now();
        let mut game = Game::new(config.clone());
        let _ = game.play(&players);
        durations.push(start.elapsed());
        turns.push(game.state.turn);
    }

    let total: u128 = durations.iter().map(|d| d.as_nanos()).sum();
    let avg_nanos = total / durations.len() as u128;
    let avg = std::time::Duration::from_nanos(avg_nanos as u64);

    let min = durations.iter().min().unwrap();
    let max = durations.iter().max().unwrap();

    println!("play timing statistics:");
    println!("  Games: {}", durations.len());
    println!("  Average: {:?}", avg);
    println!("  Min: {:?}", min);
    println!("  Max: {:?}", max);
    println!("  Average turns: {}", turns.iter().sum::<u32>() / turns.len() as u32);
    println!("  Min turns: {}", turns.iter().min().unwrap());
    println!("  Max turns: {}", turns.iter().max().unwrap());
}

fn main() {
    let args = Args::parse();

    profile_steps(args.map.clone(), args.num_players, args.num_steps, args.turns_limit, args.seed);
    profile_games(args.map.clone(), args.num_players, args.num_games, args.seed);
}
