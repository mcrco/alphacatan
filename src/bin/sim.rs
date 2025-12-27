use std::str::FromStr;
use std::time::Instant;

use catanatron_rs::MapType;
use catanatron_rs::cli::{StatisticsAccumulator, create_player, print_player_help};
use catanatron_rs::game::{Game, GameConfig};
use catanatron_rs::types::Color;
use clap::Parser;

#[derive(Debug, Parser, Clone)]
#[command(name = "catanatron-sim")]
#[command(about = "Catan Bot Simulator - Simulate games between different player strategies")]
struct Args {
    /// Number of games to play
    #[arg(short = 'n', long, default_value_t = 5)]
    num: u32,

    /// Comma-separated player codes (e.g., R,R,R,R or F,F,R,R)
    /// Use ':' to set player-specific params (e.g., F:0.1 for epsilon)
    /// Codes: R=Random, F=ValueFunction
    #[arg(long, default_value = "R,R,R,R")]
    players: String,

    /// Random seed for reproducibility
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Map type: BASE, MINI, or TOURNAMENT
    #[arg(long, default_value = "BASE")]
    map: String,

    /// Victory points needed to win
    #[arg(long, default_value_t = 10)]
    vps_to_win: u8,

    /// Show player codes and exit
    #[arg(long)]
    help_players: bool,

    /// Silence console output
    #[arg(long)]
    quiet: bool,

    /// Number of worker threads for parallel execution
    #[arg(long, default_value_t = 1)]
    workers: usize,
}

fn main() {
    let args = Args::parse();

    if args.help_players {
        print_player_help();
        return;
    }

    // Parse player codes
    let player_keys: Vec<&str> = args.players.split(',').collect();
    if player_keys.is_empty() || player_keys.len() > 4 {
        eprintln!("Error: Must specify 1-4 players");
        std::process::exit(1);
    }

    let colors = [Color::Red, Color::Blue, Color::Orange, Color::White];
    let mut players: Vec<catanatron_rs::cli::players::PlayerInstance> = Vec::new();

    for (i, key) in player_keys.iter().enumerate() {
        let parts: Vec<&str> = key.split(':').collect();
        let code = parts[0];
        let params = if parts.len() > 1 {
            parts[1..].to_vec()
        } else {
            Vec::new()
        };

        match create_player(code, colors[i], params) {
            Some(player) => players.push(player),
            None => {
                eprintln!("Error: Unknown player code '{}'", code);
                eprintln!("Use --help-players to see available codes");
                std::process::exit(1);
            }
        }
    }

    let map_type = MapType::from_str(&args.map.to_uppercase()).unwrap_or_else(|_| {
        eprintln!(
            "Error: Invalid map type '{}'. Use BASE, MINI, or TOURNAMENT",
            args.map
        );
        std::process::exit(1);
    });

    // Run simulations
    let mut stats = StatisticsAccumulator::new();

    if args.workers > 1 {
        run_parallel_simulations(&args, &players, &mut stats, map_type);
    } else {
        run_sequential_simulations(&args, &players, &mut stats, map_type);
    }

    // Print summary
    if !args.quiet {
        print_summary(&stats, &players);
    }
}

fn run_sequential_simulations(
    args: &Args,
    players: &[catanatron_rs::cli::players::PlayerInstance],
    stats: &mut StatisticsAccumulator,
    map_type: MapType,
) {
    for game_idx in 0..args.num {
        let config = GameConfig {
            num_players: players.len(),
            map_type,
            vps_to_win: args.vps_to_win,
            seed: args.seed + game_idx as u64,
        };

        let start = Instant::now();
        let mut game = Game::new(config);
        let winner = game.play(players);
        let duration = start.elapsed();

        stats.after(&game, duration);

        if !args.quiet {
            let last_n = 10;
            if game_idx < last_n || game_idx >= args.num.saturating_sub(last_n) {
                let winner_str = winner
                    .map(|c| format!("{:?}", c))
                    .unwrap_or_else(|| "None".to_string());
                let seating: String = game
                    .state
                    .players
                    .iter()
                    .map(|p| format!("{:?}", p.color))
                    .collect::<Vec<_>>()
                    .join(",");
                println!(
                    "Game {:>4}: Seating=[{}], Winner={:>6}, Turns={:>4}, Duration={:?}",
                    game_idx + 1,
                    seating,
                    winner_str,
                    game.state.turn,
                    duration
                );
            } else if (game_idx + 1) % 100 == 0 {
                print!(".");
                use std::io::Write;
                std::io::stdout().flush().unwrap();
            }
        }
    }
}

fn run_parallel_simulations(
    args: &Args,
    players: &[catanatron_rs::cli::players::PlayerInstance],
    stats: &mut StatisticsAccumulator,
    map_type: MapType,
) {
    use std::sync::Arc;
    use std::thread;

    // Clone players for each thread (they need to be owned)
    let players_vec: Vec<_> = players.iter().cloned().collect();
    let players = Arc::new(players_vec);
    let args = Arc::new(args.clone());

    let mut handles = Vec::new();
    let games_per_worker = args.num as usize / args.workers;
    let remainder = args.num as usize % args.workers;

    for worker_id in 0..args.workers {
        let players_clone = Arc::clone(&players);
        let args_clone = Arc::clone(&args);
        let map_type_clone = map_type;

        let num_games = if worker_id < remainder {
            games_per_worker + 1
        } else {
            games_per_worker
        };

        let handle = thread::spawn(move || {
            let mut local_stats = StatisticsAccumulator::new();
            let start_idx = worker_id * games_per_worker + worker_id.min(remainder);

            for local_idx in 0..num_games {
                let game_idx = start_idx + local_idx;
                let config = GameConfig {
                    num_players: players_clone.len(),
                    map_type: map_type_clone,
                    vps_to_win: args_clone.vps_to_win,
                    seed: args_clone.seed + game_idx as u64,
                };

                let start = Instant::now();
                let mut game = Game::new(config);
                let _winner = game.play(&**players_clone);
                let duration = start.elapsed();

                local_stats.after(&game, duration);
            }

            local_stats
        });

        handles.push(handle);
    }

    // Collect and merge results
    for handle in handles {
        let worker_stats = handle.join().unwrap();
        // Merge stats
        for (color, wins) in worker_stats.stats.wins {
            *stats.stats.wins.entry(color).or_insert(0) += wins;
        }
        for (color, vps) in worker_stats.stats.results_by_player {
            stats
                .stats
                .results_by_player
                .entry(color)
                .or_insert_with(Vec::new)
                .extend(vps);
        }
        stats.stats.games += worker_stats.stats.games;
        stats.stats.total_ticks += worker_stats.stats.total_ticks;
        stats.stats.total_turns += worker_stats.stats.total_turns;
        stats.stats.total_duration += worker_stats.stats.total_duration;
    }
}

fn print_summary(
    stats: &StatisticsAccumulator,
    players: &[catanatron_rs::cli::players::PlayerInstance],
) {
    println!("\n{}", "=".repeat(80));
    println!("SIMULATION SUMMARY");
    println!("{}", "=".repeat(80));

    // Player Summary
    println!("\nPlayer Summary:");
    println!(
        "{:<15} {:<10} {:<12} {:<12}",
        "Player", "Wins", "Win Rate", "Avg VP"
    );
    println!("{}", "-".repeat(50));

    for (idx, player) in players.iter().enumerate() {
        let color = match player {
            catanatron_rs::cli::players::PlayerInstance::Random(_) => {
                [Color::Red, Color::Blue, Color::Orange, Color::White][idx]
            }
            catanatron_rs::cli::players::PlayerInstance::ValueFunction(p) => p.color,
            catanatron_rs::cli::players::PlayerInstance::MCTS(p) => p.color,
        };

        let wins = stats.stats.wins.get(&color).copied().unwrap_or(0);
        let win_rate = if stats.stats.games > 0 {
            (wins as f64 / stats.stats.games as f64) * 100.0
        } else {
            0.0
        };

        let avg_vps = stats
            .stats
            .results_by_player
            .get(&color)
            .map(|vps| {
                if vps.is_empty() {
                    0.0
                } else {
                    vps.iter().sum::<u8>() as f64 / vps.len() as f64
                }
            })
            .unwrap_or(0.0);

        let player_name = match player {
            catanatron_rs::cli::players::PlayerInstance::Random(_) => "Random",
            catanatron_rs::cli::players::PlayerInstance::ValueFunction(_) => "ValueFunction",
            catanatron_rs::cli::players::PlayerInstance::MCTS(_) => "MCTS",
        };

        println!(
            "{:<15} {:<10} {:<11.1}% {:<12.2}",
            format!("{} ({:?})", player_name, color),
            wins,
            win_rate,
            avg_vps
        );
    }

    // Game Summary
    println!("\nGame Summary:");
    println!("  Total Games: {}", stats.stats.games);
    println!("  Avg Turns: {:.2}", stats.stats.get_avg_turns());
    println!("  Avg Ticks: {:.2}", stats.stats.get_avg_ticks());
    println!("  Avg Duration: {:.2?}", stats.stats.get_avg_duration());
}
