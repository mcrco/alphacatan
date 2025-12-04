use crate::game::action::GameAction;
use crate::game::game::Game;
use crate::game::players::PlayerState;
use crate::players::BasePlayer;
use crate::types::Color;
use rand::{Rng, seq::SliceRandom};

#[derive(Clone)]
pub struct ValueFunctionPlayer {
    pub color: Color,
    pub params: ValueFunctionParams,
    pub epsilon: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ValueFunctionParams {
    pub public_vps: f64,
    pub production: f64,
    pub enemy_production: f64,
    pub num_tiles: f64,
    pub reachable_production_0: f64,
    pub reachable_production_1: f64,
    pub buildable_nodes: f64,
    pub longest_road: f64,
    pub hand_synergy: f64,
    pub hand_resources: f64,
    pub discard_penalty: f64,
    pub hand_devs: f64,
    pub army_size: f64,
}

impl Default for ValueFunctionParams {
    fn default() -> Self {
        Self {
            public_vps: 3e14,
            production: 1e8,
            enemy_production: -1e8,
            num_tiles: 1.0,
            reachable_production_0: 0.0,
            reachable_production_1: 1e4,
            buildable_nodes: 1e3,
            longest_road: 10.0,
            hand_synergy: 1e2,
            hand_resources: 1.0,
            discard_penalty: -5.0,
            hand_devs: 10.0,
            army_size: 10.1,
        }
    }
}

impl ValueFunctionPlayer {
    pub fn new(color: Color, params: Option<ValueFunctionParams>, epsilon: Option<f64>) -> Self {
        Self {
            color,
            params: params.unwrap_or_default(),
            epsilon,
        }
    }
}

impl BasePlayer for ValueFunctionPlayer {
    fn decide(&self, game: &Game, actions: &[GameAction]) -> Option<GameAction> {
        if actions.len() == 1 {
            return actions.first().cloned();
        }

        // Epsilon-greedy exploration
        if let Some(epsilon) = self.epsilon {
            let mut rng = rand::thread_rng();
            if rng.gen_bool(epsilon) {
                return actions.choose(&mut rng).cloned();
            }
        }

        // Find player index
        let player_idx = game
            .state
            .players
            .iter()
            .position(|p| p.color == self.color)?;

        // Evaluate each action (must match Python implementation exactly)
        let mut best_value = f64::NEG_INFINITY;
        let mut best_action = None;

        for action in actions {
            let mut game_copy = game.copy();
            game_copy.execute(action.clone());

            let value = evaluate_state(&game_copy, player_idx, &self.params);
            if value > best_value {
                best_value = value;
                best_action = Some(action.clone());
            }
        }

        best_action
    }
}

fn evaluate_state(game: &Game, player_idx: usize, params: &ValueFunctionParams) -> f64 {
    let player = &game.state.players[player_idx];
    let total_vps = player.total_points() as f64;

    // Production (simplified - would need feature extraction for full implementation)
    let production = calculate_production(game, player_idx);
    let enemy_production = calculate_enemy_production(game, player_idx);

    // Longest road
    let longest_road_length = calculate_longest_road_length(game, player_idx) as f64;

    // Buildable nodes (simplified)
    let buildable_nodes = count_buildable_nodes(game, player_idx) as f64;

    // Hand resources
    let hand_resources = player.resources.total() as f64;
    let hand_devs = (player.dev_cards.len() + player.fresh_dev_cards.len()) as f64;

    // Discard penalty
    let discard_penalty = if hand_resources > 7.0 {
        params.discard_penalty
    } else {
        0.0
    };

    // Hand synergy (simplified)
    let hand_synergy = calculate_hand_synergy(player);

    // Number of tiles controlled
    let num_tiles = count_controlled_tiles(game, player_idx) as f64;

    // Army size (knights played)
    let army_size = player
        .dev_cards
        .iter()
        .filter(|card| matches!(card, crate::types::DevelopmentCard::Knight))
        .count() as f64;

    // Reachable production (simplified - would need full feature extraction)
    let reachable_production_0 = 0.0; // Would need reachability features
    let reachable_production_1 = 0.0; // Would need reachability features

    let longest_road_factor = if buildable_nodes == 0.0 {
        params.longest_road
    } else {
        0.1
    };

    total_vps * params.public_vps
        + production * params.production
        + enemy_production * params.enemy_production
        + reachable_production_0 * params.reachable_production_0
        + reachable_production_1 * params.reachable_production_1
        + hand_synergy * params.hand_synergy
        + buildable_nodes * params.buildable_nodes
        + num_tiles * params.num_tiles
        + hand_resources * params.hand_resources
        + discard_penalty
        + longest_road_length * longest_road_factor
        + hand_devs * params.hand_devs
        + army_size * params.army_size
}

fn calculate_production(game: &Game, player_idx: usize) -> f64 {
    let player = &game.state.players[player_idx];
    let mut production = 0.0;

    // Get all nodes with buildings
    let mut owned_nodes = player.settlements.clone();
    owned_nodes.extend(&player.cities);

    for node_id in owned_nodes {
        if let Some(tile_ids) = game.state.map.adjacent_tiles.get(&node_id) {
            for tile_id in tile_ids {
                if let Some(tile) = game.state.map.tiles_by_id.get(tile_id) {
                    if let (Some(_resource), Some(number)) = (tile.resource, tile.number) {
                        let proba = number_probability(number);
                        production += proba;
                    }
                }
            }
        }
    }

    production
}

fn calculate_enemy_production(game: &Game, player_idx: usize) -> f64 {
    let mut total = 0.0;
    for idx in 0..game.state.players.len() {
        if idx != player_idx {
            total += calculate_production(game, idx);
        }
    }
    total
}

fn calculate_longest_road_length(game: &Game, player_idx: usize) -> usize {
    // Simplified - would need full longest road calculation
    game.state.players[player_idx].roads.len()
}

fn count_buildable_nodes(game: &Game, player_idx: usize) -> usize {
    // Simplified - would need full validation logic
    let player = &game.state.players[player_idx];
    let mut count = 0;
    for node_id in &game.state.map.land_nodes {
        if !player.settlements.contains(node_id) && !player.cities.contains(node_id) {
            // Check if node is too close to other buildings (simplified)
            let mut too_close = false;
            for other_node in &game.state.map.land_nodes {
                if *other_node != *node_id {
                    // Check if nodes are adjacent (simplified check)
                    if are_nodes_adjacent(game, *node_id, *other_node) {
                        if game.state.node_occupancy.contains_key(other_node) {
                            too_close = true;
                            break;
                        }
                    }
                }
            }
            if !too_close {
                count += 1;
            }
        }
    }
    count
}

fn are_nodes_adjacent(game: &Game, node_a: crate::board::NodeId, node_b: crate::board::NodeId) -> bool {
    // Check if nodes share an edge
    for edge in game.state.map.node_edges.get(&node_a).unwrap_or(&vec![]) {
        if edge.0 == node_b || edge.1 == node_b {
            return true;
        }
    }
    false
}

fn calculate_hand_synergy(player: &PlayerState) -> f64 {
    // Simplified hand synergy calculation
    let wheat = player.resources.get(crate::types::Resource::Wheat);
    let ore = player.resources.get(crate::types::Resource::Ore);
    let sheep = player.resources.get(crate::types::Resource::Sheep);
    let brick = player.resources.get(crate::types::Resource::Brick);
    let wood = player.resources.get(crate::types::Resource::Wood);

    let distance_to_city = ((2.0 - wheat as f64).max(0.0) + (3.0 - ore as f64).max(0.0)) / 5.0;
    let distance_to_settlement = ((1.0 - wheat as f64).max(0.0)
        + (1.0 - sheep as f64).max(0.0)
        + (1.0 - brick as f64).max(0.0)
        + (1.0 - wood as f64).max(0.0))
        / 4.0;

    (2.0 - distance_to_city - distance_to_settlement) / 2.0
}

fn count_controlled_tiles(game: &Game, player_idx: usize) -> usize {
    let player = &game.state.players[player_idx];
    let mut owned_tiles = std::collections::HashSet::new();

    let mut owned_nodes = player.settlements.clone();
    owned_nodes.extend(&player.cities);

    for node_id in owned_nodes {
        if let Some(tile_ids) = game.state.map.adjacent_tiles.get(&node_id) {
            for tile_id in tile_ids {
                owned_tiles.insert(*tile_id);
            }
        }
    }

    owned_tiles.len()
}

fn number_probability(number: u8) -> f64 {
    // Probability of rolling this number with two dice
    match number {
        2 | 12 => 1.0 / 36.0,
        3 | 11 => 2.0 / 36.0,
        4 | 10 => 3.0 / 36.0,
        5 | 9 => 4.0 / 36.0,
        6 | 8 => 5.0 / 36.0,
        7 => 6.0 / 36.0,
        _ => 0.0,
    }
}


