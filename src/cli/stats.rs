use std::collections::HashMap;
use std::time::Duration;

use crate::game::game::Game;
use crate::types::Color;

#[derive(Debug, Default, Clone)]
pub struct GameStats {
    pub wins: HashMap<Color, u32>,
    pub results_by_player: HashMap<Color, Vec<u8>>,
    pub games: u32,
    pub total_ticks: u64,
    pub total_turns: u64,
    pub total_duration: Duration,
}

impl GameStats {
    pub fn new() -> Self {
        Self {
            wins: HashMap::new(),
            results_by_player: HashMap::new(),
            total_ticks: 0,
            total_turns: 0,
            total_duration: Duration::ZERO,
            games: 0,
        }
    }

    pub fn record_game(&mut self, game: &Game, duration: Duration) {
        self.games += 1;
        self.total_duration += duration;
        self.total_turns += game.state.turn as u64;
        self.total_ticks += game.state.actions.len() as u64;

        if let Some(winner) = game.winning_color() {
            *self.wins.entry(winner).or_insert(0) += 1;
        }

        for player in &game.state.players {
            let vps = player.total_points();
            self.results_by_player
                .entry(player.color)
                .or_insert_with(Vec::new)
                .push(vps);
        }
    }

    pub fn get_avg_ticks(&self) -> f64 {
        if self.games == 0 {
            return 0.0;
        }
        self.total_ticks as f64 / self.games as f64
    }

    pub fn get_avg_turns(&self) -> f64 {
        if self.games == 0 {
            return 0.0;
        }
        self.total_turns as f64 / self.games as f64
    }

    pub fn get_avg_duration(&self) -> Duration {
        if self.games == 0 {
            return Duration::ZERO;
        }
        self.total_duration / self.games
    }
}

pub struct StatisticsAccumulator {
    pub stats: GameStats,
}

impl StatisticsAccumulator {
    pub fn new() -> Self {
        Self {
            stats: GameStats::new(),
        }
    }

    pub fn after(&mut self, game: &Game, duration: Duration) {
        self.stats.record_game(game, duration);
    }
}
