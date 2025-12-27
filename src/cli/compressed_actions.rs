use std::collections::HashMap;

use crate::game::action::{ActionPayload, GameAction};
use crate::types::ActionType;

#[derive(Debug, Clone)]
pub struct CompressedActionGroup {
    pub action_type: ActionType,
    pub description: String,
    pub actions: Vec<(usize, GameAction)>, // (original_index, action)
}

pub fn compress_actions(actions: &[GameAction]) -> Vec<CompressedActionGroup> {
    let mut groups: HashMap<String, CompressedActionGroup> = HashMap::new();

    for (idx, action) in actions.iter().enumerate() {
        let key = group_key(action);
        let description = group_description(action);

        let group = groups.entry(key).or_insert_with(|| CompressedActionGroup {
            action_type: action.action_type,
            description,
            actions: Vec::new(),
        });

        group.actions.push((idx, action.clone()));
    }

    // Sort actions within each group by their detailed description for consistent ordering
    for group in groups.values_mut() {
        group
            .actions
            .sort_by(|(_, a), (_, b)| action_detail_label(a).cmp(&action_detail_label(b)));
    }

    // Sort groups purely lexicographically by their description
    let mut groups: Vec<_> = groups.into_values().collect();
    groups.sort_by(|a, b| a.description.cmp(&b.description));

    groups
}

fn group_key(action: &GameAction) -> String {
    match action.action_type {
        ActionType::BuildRoad => "BuildRoad".to_string(),
        ActionType::BuildSettlement => "BuildSettlement".to_string(),
        ActionType::BuildCity => "BuildCity".to_string(),
        ActionType::MaritimeTrade => {
            // Group by give/receive pattern
            if let ActionPayload::MaritimeTrade { give, receive } = &action.payload {
                format!("MaritimeTrade:{:?}:{:?}", summarize_bundle(give), receive)
            } else {
                "MaritimeTrade".to_string()
            }
        }
        ActionType::PlayYearOfPlenty => {
            // Group by resource bundle pattern
            if let ActionPayload::Resources(bundle) = &action.payload {
                format!("PlayYearOfPlenty:{}", summarize_bundle(bundle))
            } else {
                "PlayYearOfPlenty".to_string()
            }
        }
        ActionType::PlayMonopoly => {
            if let ActionPayload::Resource(res) = &action.payload {
                format!("PlayMonopoly:{:?}", res)
            } else {
                "PlayMonopoly".to_string()
            }
        }
        ActionType::PlayKnightCard => "PlayKnightCard".to_string(),
        ActionType::MoveRobber => {
            // Group by tile
            if let ActionPayload::Robber { tile_id, .. } = &action.payload {
                format!("MoveRobber:{}", tile_id)
            } else {
                "MoveRobber".to_string()
            }
        }
        ActionType::Discard => {
            if let ActionPayload::Resource(res) = &action.payload {
                format!("Discard:{:?}", res)
            } else if let ActionPayload::Resources(bundle) = &action.payload {
                format!("Discard:{}", summarize_bundle(bundle))
            } else {
                "Discard".to_string()
            }
        }
        ActionType::OfferTrade => "OfferTrade".to_string(),
        _ => format!("{:?}", action.action_type),
    }
}

fn group_description(action: &GameAction) -> String {
    match action.action_type {
        ActionType::Roll => "Roll Dice".to_string(),
        ActionType::EndTurn => "End Turn".to_string(),
        ActionType::BuildRoad => "Build Road".to_string(),
        ActionType::BuildSettlement => "Build Settlement".to_string(),
        ActionType::BuildCity => "Build City".to_string(),
        ActionType::BuyDevelopmentCard => "Buy Development Card".to_string(),
        ActionType::PlayKnightCard => "Play Knight Card".to_string(),
        ActionType::PlayYearOfPlenty => {
            if let ActionPayload::Resources(bundle) = &action.payload {
                format!("Play Year of Plenty - get {}", summarize_bundle(bundle))
            } else {
                "Play Year of Plenty".to_string()
            }
        }
        ActionType::PlayMonopoly => {
            if let ActionPayload::Resource(res) = &action.payload {
                format!("Play Monopoly - take all {:?}", res)
            } else {
                "Play Monopoly".to_string()
            }
        }
        ActionType::PlayRoadBuilding => "Play Road Building".to_string(),
        ActionType::MaritimeTrade => {
            if let ActionPayload::MaritimeTrade { give, receive } = &action.payload {
                format!(
                    "Maritime Trade - give {}, receive {:?}",
                    summarize_bundle(give),
                    receive
                )
            } else {
                "Maritime Trade".to_string()
            }
        }
        ActionType::MoveRobber => {
            if let ActionPayload::Robber { tile_id, .. } = &action.payload {
                format!("Move Robber to tile {}", tile_id)
            } else {
                "Move Robber".to_string()
            }
        }
        ActionType::Discard => {
            if let ActionPayload::Resource(res) = &action.payload {
                format!("Discard {:?}", res)
            } else if let ActionPayload::Resources(bundle) = &action.payload {
                format!("Discard {}", summarize_bundle(bundle))
            } else {
                "Discard".to_string()
            }
        }
        ActionType::OfferTrade => "Offer Trade".to_string(),
        ActionType::AcceptTrade => "Accept Trade".to_string(),
        ActionType::RejectTrade => "Reject Trade".to_string(),
        ActionType::ConfirmTrade => "Confirm Trade".to_string(),
        ActionType::CancelTrade => "Cancel Trade".to_string(),
    }
}

fn summarize_bundle(bundle: &crate::game::resources::ResourceBundle) -> String {
    let parts: Vec<String> = bundle
        .iter()
        .filter(|(_, count)| *count > 0)
        .map(|(res, count)| format!("{}x{:?}", count, res))
        .collect();
    if parts.is_empty() {
        "nothing".to_string()
    } else {
        parts.join(",")
    }
}

pub fn display_compressed_actions(groups: &[CompressedActionGroup]) -> HashMap<usize, usize> {
    // Maps displayed_index -> original_index
    let mut index_map = HashMap::new();
    let mut displayed_idx = 0;

    println!("\n{}", "-".repeat(80));
    println!("AVAILABLE ACTIONS:");
    println!("{}", "-".repeat(80));

    for (group_idx, group) in groups.iter().enumerate() {
        if group.actions.len() == 1 {
            // Single action - show directly
            let (original_idx, _) = &group.actions[0];
            println!("[{}] {}", displayed_idx, group.description);
            index_map.insert(displayed_idx, *original_idx);
            displayed_idx += 1;
        } else {
            // Multiple actions - show grouped
            println!(
                "[{}] {} ({} options) - use 'e{}' to expand",
                displayed_idx,
                group.description,
                group.actions.len(),
                group_idx
            );
            index_map.insert(displayed_idx, group_idx); // Store group index for expansion
            displayed_idx += 1;
        }
    }

    index_map
}

pub fn expand_group(group: &CompressedActionGroup, start_index: usize) -> HashMap<usize, usize> {
    let mut index_map = HashMap::new();

    // Build mapping without printing - TUI will handle display
    for (i, (original_idx, _action)) in group.actions.iter().enumerate() {
        let display_idx = start_index + i;
        index_map.insert(display_idx, *original_idx);
    }

    index_map
}

pub fn action_detail_label(action: &GameAction) -> String {
    match action.action_type {
        ActionType::Roll => {
            if let ActionPayload::Dice(d1, d2) = &action.payload {
                let sum = (*d1 as u16) + (*d2 as u16);
                format!("Rolled {} + {} = {}", d1, d2, sum)
            } else {
                group_description(action)
            }
        }
        ActionType::BuildRoad => {
            if let ActionPayload::Edge(edge) = &action.payload {
                format!("Edge ({}, {})", edge.0, edge.1)
            } else {
                "Road".to_string()
            }
        }
        ActionType::BuildSettlement => {
            if let ActionPayload::Node(node) = &action.payload {
                format!("Node {}", node)
            } else {
                "Settlement".to_string()
            }
        }
        ActionType::BuildCity => {
            if let ActionPayload::Node(node) = &action.payload {
                format!("Node {}", node)
            } else {
                "City".to_string()
            }
        }
        ActionType::MoveRobber => {
            if let ActionPayload::Robber {
                tile_id,
                victim,
                resource,
            } = &action.payload
            {
                let parts: Vec<String> = vec![
                    Some(format!("tile {}", tile_id)),
                    victim.map(|v| format!("victim={}", v)),
                    resource.map(|r| format!("resource={:?}", r)),
                ]
                .into_iter()
                .flatten()
                .collect();
                parts.join(", ")
            } else {
                "Move Robber".to_string()
            }
        }
        ActionType::Discard => {
            if let ActionPayload::Resource(res) = &action.payload {
                format!("Discard {:?}", res)
            } else if let ActionPayload::Resources(bundle) = &action.payload {
                format!("Discard {}", summarize_bundle(bundle))
            } else {
                "Discard".to_string()
            }
        }
        _ => group_description(action),
    }
}
