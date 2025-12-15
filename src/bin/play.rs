use std::str::FromStr;

use clap::Parser;
use catanatron_rs::cli::{create_player, print_player_help, HumanPlayer};
use catanatron_rs::cli::players::PlayerInstance;
use catanatron_rs::game::action::GameAction;
use catanatron_rs::game::{Game, GameConfig};
use catanatron_rs::players::BasePlayer;
use catanatron_rs::types::Color;
use catanatron_rs::MapType;

#[derive(Clone)]
enum UnifiedPlayer {
    Human(HumanPlayer),
    Bot(PlayerInstance),
}

impl BasePlayer for UnifiedPlayer {
    fn decide(&self, game: &Game, actions: &[GameAction]) -> Option<GameAction> {
        match self {
            UnifiedPlayer::Human(p) => p.decide(game, actions),
            UnifiedPlayer::Bot(p) => p.decide(game, actions),
        }
    }
}

#[derive(Debug, Parser, Clone)]
#[command(name = "catanatron-play")]
#[command(about = "Play Catan 1v1 against a bot")]
struct Args {
    /// Bot player code (R=Random, F=ValueFunction, M=MCTS)
    #[arg(short = 'b', long, default_value = "F")]
    bot: String,

    /// Bot-specific parameters (comma-separated, e.g., for MCTS: "100,true")
    #[arg(long, default_value = "")]
    bot_params: String,

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
}

fn main() {
    let args = Args::parse();

    if args.help_players {
        print_player_help();
        return;
    }

    // Create bot player
    let bot_params: Vec<&str> = if args.bot_params.is_empty() {
        Vec::new()
    } else {
        args.bot_params.split(',').collect()
    };

    let bot = match create_player(&args.bot, Color::Blue, bot_params) {
        Some(player) => UnifiedPlayer::Bot(player),
        None => {
            eprintln!("Error: Unknown bot code '{}'", args.bot);
            eprintln!("Use --help-players to see available codes");
            std::process::exit(1);
        }
    };

    // Create human player (always Red)
    let human = UnifiedPlayer::Human(HumanPlayer::new(Color::Red));
    
    // Create players array: human is always player 0 (Red), bot is player 1 (Blue)
    let players = vec![human, bot];

    let map_type = MapType::from_str(&args.map.to_uppercase())
        .unwrap_or_else(|_| {
            eprintln!("Error: Invalid map type '{}'. Use BASE, MINI, or TOURNAMENT", args.map);
            std::process::exit(1);
        });

    // Create game config for 2 players
    let config = GameConfig {
        num_players: 2,
        map_type,
        vps_to_win: args.vps_to_win,
        seed: args.seed,
    };

    println!("Starting game: You (Red) vs Bot (Blue)");
    println!("Map: {:?}, Victory Points to Win: {}", map_type, args.vps_to_win);
    println!("{}", "=".repeat(80));

    // Create game
    let mut game = Game::new(config);

    // Game loop
    loop {
        // Check for winner
        if let Some(winner_color) = game.winning_color() {
            println!("\n{}", "=".repeat(80));
            if winner_color == Color::Red {
                println!("🎉 YOU WIN! 🎉");
            } else {
                println!("🤖 Bot wins. Better luck next time!");
            }
            println!("{}", "=".repeat(80));
            break;
        }

        // Check turn limit
        if game.state.turn >= 1000 {
            println!("\nGame reached turn limit. No winner declared.");
            break;
        }

        let current_idx = game.state.current_player;
        let is_human_turn = current_idx == 0; // Human is always player 0 (Red)

        if is_human_turn {
            // Human player's turn - show nothing before, display happens in HumanPlayer
        } else {
            // Bot player's turn
            println!("\n🤖 Bot is thinking...");
        }

        if let Some(action) = game.play_tick(&players) {
            if is_human_turn {
                println!("\n→ You played: {:?}", action.action_type);
            } else {
                println!("→ Bot played: {:?}", action.action_type);
            }
        }
    }

    // Final stats
    println!("\n{}", "=".repeat(80));
    println!("FINAL STATS:");
    println!("{}", "=".repeat(80));
    
    for (idx, player) in game.state.players.iter().enumerate() {
        let label = if idx == 0 { "YOU" } else { "BOT" };
        println!("\n{} ({:?}):", label, player.color);
        println!("  Victory Points: {}", player.total_points());
        println!("  Resources: {}", player.resources);
        println!("  Settlements: {}", player.settlements.len());
        println!("  Cities: {}", player.cities.len());
        println!("  Roads: {}", player.roads.len());
    }
    println!("\nTotal Turns: {}", game.state.turn);
}

