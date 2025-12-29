use std::str::FromStr;
use std::time::Instant;

use catanatron_rs::MapType;
use catanatron_rs::game::{Game, GameConfig};
use catanatron_rs::players::{RandomPlayer};

use clap::Parser;

#[derive(Debug, Parser, Clone)]
#[command(name = "settle-rs-profile-game")]
#[command(about = "Profile the game")]
struct Args {
    #[arg(long, default_value = "100000")]
    num_steps: u32,

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

fn main() {
    let args = Args::parse();

    let map_type = MapType::from_str(&args.map.to_uppercase()).unwrap_or_else(|_| {
        eprintln!("Error: Invalid map type '{}'. Use BASE, MINI, or TOURNAMENT", args.map);
        std::process::exit(1);
    });

    let mut config = GameConfig {
        num_players: args.num_players,
        map_type,
        vps_to_win: 10,
        seed: args.seed,
    };

    let mut game = Game::new(config.clone());
    let mut players = Vec::new();
    for _i in 0..args.num_players {
        players.push(RandomPlayer);
    }
    
    let mut durations = Vec::new();
    for _ in 0..args.num_steps {
        if game.winning_color().is_some() {
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