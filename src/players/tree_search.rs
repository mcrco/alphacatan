use std::collections::HashSet;

use crate::game::{
    action::{ActionPayload, GameAction},
    game::Game,
    state::{GamePhase, GameState, Structure},
};
use crate::types::{ActionPrompt, ActionType, Color, Resource};

fn number_probability(number: u8) -> f64 {
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

fn execute_deterministic(game: &Game, action: &GameAction) -> Vec<(Game, f64)> {
    let mut next = game.copy();
    let a = action.clone();
    if next.state.step(a).is_ok() {
        vec![(next, 1.0)]
    } else {
        Vec::new()
    }
}

fn execute_roll(game: &Game, action: &GameAction) -> Vec<(Game, f64)> {
    let mut outcomes = Vec::new();
    for sum in 2u8..=12 {
        let p = number_probability(sum);
        if p == 0.0 {
            continue;
        }
        // Same mapping as Python: (roll//2, ceil(roll/2))
        let d1 = sum / 2;
        let d2 = if sum % 2 == 0 { sum / 2 } else { sum / 2 + 1 };
        let mut next = game.copy();
        let mut a = action.clone();
        a.payload = ActionPayload::Dice(d1, d2);
        if next.state.step(a).is_ok() {
            outcomes.push((next, p));
        }
    }
    outcomes
}

fn execute_buy_development(game: &Game, action: &GameAction) -> Vec<(Game, f64)> {
    // Python uses an inferred dev-deck distribution based on hidden information.
    // Our Bank API does not expose the dev deck composition, so we approximate
    // this as a single deterministic branch and let GameState handle the draw.
    execute_deterministic(game, action)
}

fn execute_move_robber(game: &Game, action: &GameAction) -> Vec<(Game, f64)> {
    let mut outcomes = Vec::new();
    let state = &game.state;

    let (tile_id, victim_opt) = match action.payload {
        ActionPayload::Robber {
            tile_id, victim, ..
        } => (tile_id, victim),
        _ => return execute_deterministic(game, action),
    };

    // If there is no victim, the move is deterministic.
    if victim_opt.is_none() {
        return execute_deterministic(game, action);
    }

    let victim_idx = victim_opt.unwrap();
    if victim_idx >= state.players.len() {
        return execute_deterministic(game, action);
    }

    let victim = &state.players[victim_idx];
    // Count total cards
    let mut total_cards: u32 = 0;
    for r in Resource::ALL {
        total_cards += victim.resources.get(r) as u32;
    }
    if total_cards == 0 {
        return execute_deterministic(game, action);
    }

    // Enumerate possible stolen resources (uniform over resource types present)
    let mut candidate_resources = Vec::new();
    for r in Resource::ALL {
        if victim.resources.get(r) > 0 {
            candidate_resources.push(r);
        }
    }
    if candidate_resources.is_empty() {
        return execute_deterministic(game, action);
    }

    let p = 1.0 / (candidate_resources.len() as f64);
    for res in candidate_resources {
        let mut next = game.copy();
        let mut a = action.clone();
        a.payload = ActionPayload::Robber {
            tile_id,
            victim: Some(victim_idx),
            resource: Some(res),
        };
        if next.state.step(a).is_ok() {
            outcomes.push((next, p));
        }
    }

    outcomes
}

/// Mirror of Python `execute_spectrum`: expand a Game+Action into one or more
/// possible successor states, each with an associated probability.
pub fn execute_spectrum(game: &Game, action: &GameAction) -> Vec<(Game, f64)> {
    match action.action_type {
        ActionType::Roll => execute_roll(game, action),
        ActionType::BuildSettlement
        | ActionType::BuildRoad
        | ActionType::BuildCity
        | ActionType::EndTurn
        | ActionType::PlayKnightCard
        | ActionType::PlayYearOfPlenty
        | ActionType::PlayRoadBuilding
        | ActionType::MaritimeTrade
        | ActionType::Discard
        | ActionType::OfferTrade
        | ActionType::AcceptTrade
        | ActionType::RejectTrade
        | ActionType::ConfirmTrade
        | ActionType::CancelTrade => execute_deterministic(game, action),
        ActionType::BuyDevelopmentCard => execute_buy_development(game, action),
        ActionType::MoveRobber => execute_move_robber(game, action),
        ActionType::PlayMonopoly => execute_deterministic(game, action),
    }
}

fn player_has_port(state: &GameState, player_idx: usize, port: Option<Resource>) -> bool {
    if let Some(nodes) = state.map.port_nodes.get(&port) {
        nodes
            .iter()
            .any(|node| match state.node_occupancy.get(node) {
                Some(Structure::Settlement { player }) | Some(Structure::City { player }) => {
                    *player == player_idx
                }
                _ => false,
            })
    } else {
        false
    }
}

fn maritime_rate(state: &GameState, player_idx: usize, resource: Resource) -> u8 {
    if player_has_port(state, player_idx, Some(resource)) {
        return 2;
    }
    if player_has_port(state, player_idx, None) {
        return 3;
    }
    4
}

/// Rough mirror of Python `list_prunned_actions`. We implement the same
/// high-level pruning rules:
/// - During initial settlement placement, prune 1-tile locations.
/// - When a 3:1 port is available, prune clearly dominated 4:1 maritime trades.
/// Robber pruning based on production-impact is intentionally omitted.
pub fn list_pruned_actions(game: &Game) -> Vec<GameAction> {
    let state: &GameState = &game.state;
    let mut actions: Vec<GameAction> = state.legal_actions().to_vec();

    if actions.is_empty() {
        return actions;
    }

    let current_player = state.current_player;
    let _current_color: Color = state.players[current_player].color;

    let mut types = HashSet::new();
    for a in &actions {
        types.insert(a.action_type);
    }

    // 1) Prune initial settlements at 1-tile places
    if types.contains(&ActionType::BuildSettlement)
        && matches!(state.phase, GamePhase::Setup(_))
        && matches!(state.pending_prompt, ActionPrompt::BuildInitialSettlement)
    {
        actions = actions
            .into_iter()
            .filter(|a| {
                if a.action_type != ActionType::BuildSettlement {
                    return true;
                }
                match a.payload {
                    ActionPayload::Node(node) => {
                        if let Some(adj) = state.map.adjacent_tiles.get(&node) {
                            adj.len() != 1
                        } else {
                            true
                        }
                    }
                    _ => true,
                }
            })
            .collect();
    }

    // 2) Prune maritime trades when a 3:1 port is available: drop 4:1 trades.
    if types.contains(&ActionType::MaritimeTrade) {
        let has_three_to_one = player_has_port(state, current_player, None);
        if has_three_to_one {
            let mut pruned = Vec::with_capacity(actions.len());
            for a in actions.into_iter() {
                if a.action_type != ActionType::MaritimeTrade {
                    pruned.push(a);
                    continue;
                }
                let give_bundle = match &a.payload {
                    ActionPayload::MaritimeTrade { give, .. } => give,
                    _ => {
                        pruned.push(a);
                        continue;
                    }
                };
                // Identify which resource is being given (there should be exactly one)
                let mut given: Option<Resource> = None;
                for r in Resource::ALL {
                    if give_bundle.get(r) > 0 {
                        given = Some(r);
                        break;
                    }
                }
                if let Some(resource) = given {
                    if maritime_rate(state, current_player, resource) == 4 {
                        // 4:1 trade while a 3:1 port exists: prune
                        continue;
                    }
                }
                pruned.push(a);
            }
            actions = pruned;
        }
    }

    // 3) Robber pruning based on production impact is not implemented here.
    // All robber actions are kept. This preserves core game legality while
    // simplifying the search heuristic.

    actions
}
