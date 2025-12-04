use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use once_cell::sync::Lazy;

use crate::{
    board::{CatanMap, EdgeId, NodeId},
    coords::{CubeCoord, offset_to_cube},
    game::{
        players::{MAX_CITIES, MAX_ROADS, MAX_SETTLEMENTS, PlayerState},
        state::{GameState, Structure},
    },
    types::{ActionPrompt, DevelopmentCard, Resource},
};

const WIDTH: usize = 21;
const HEIGHT: usize = 11;

const PAIRS: &[(NodeId, NodeId)] = &[(82, 93), (79, 94), (42, 25), (41, 26), (73, 59), (72, 60)];

fn is_graph_feature(name: &str) -> bool {
    name.starts_with("NODE")
        || name.starts_with("EDGE")
        || name.starts_with("TILE")
        || name.starts_with("PORT")
}

#[derive(Debug, Clone)]
pub struct FeatureCollection {
    pub names: Vec<String>,
    pub values: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct BoardTensor {
    pub width: usize,
    pub height: usize,
    pub channels: usize,
    pub data: Vec<f32>,
}

impl FeatureCollection {
    pub fn numeric_values(&self) -> Vec<f32> {
        self.names
            .iter()
            .zip(self.values.iter())
            .filter(|(name, _)| !is_graph_feature(name))
            .map(|(_, value)| *value)
            .collect()
    }
}

pub fn collect_features(game: &GameState, perspective: usize) -> FeatureCollection {
    let mut features = BTreeMap::new();
    let order = iter_players(game, perspective);

    gather_player_features(game, &order, &mut features);
    gather_resource_hand_features(&order, &mut features);
    gather_tile_features(game, &mut features);
    gather_port_features(game, &mut features);
    gather_graph_features(game, &order, &mut features);
    gather_game_features(game, &mut features);

    let (names, values): (Vec<_>, Vec<_>) =
        features.into_iter().map(|(k, v)| (k, v as f32)).unzip();

    FeatureCollection { names, values }
}

pub fn build_board_tensor(game: &GameState, perspective: usize) -> BoardTensor {
    let order = iter_players(game, perspective);
    let num_players = order.len();
    let channels = 2 * num_players + 5 + 1 + 6;
    let mut data = vec![0.0; WIDTH * HEIGHT * channels];

    let node_map = node_position_map();
    let edge_map = edge_position_map();
    let tile_map = tile_coordinate_map();

    for (relative_idx, (_, player)) in order.iter().enumerate() {
        for node in &player.settlements {
            if let Some(&(x, y)) = node_map.get(node) {
                set_value(&mut data, relative_idx * 2, x, y, 1.0);
            }
        }
        for node in &player.cities {
            if let Some(&(x, y)) = node_map.get(node) {
                set_value(&mut data, relative_idx * 2, x, y, 2.0);
            }
        }
        for edge in &player.roads {
            let normalized = normalize_edge(*edge);
            if let Some(&(x, y)) = edge_map.get(&normalized) {
                set_value(&mut data, relative_idx * 2 + 1, x, y, 1.0);
            }
        }
    }

    for (coord, tile) in &game.map.land_tiles {
        if let Some(resource) = tile.resource {
            if let Some(&(x, y)) = tile_map.get(coord) {
                let proba = tile.number.map(number_probability).unwrap_or(0.0);
                let channel = 2 * num_players + resource_index(resource);
                stamp_tile(&mut data, channel, x, y, proba);
            }
        }
    }

    if let Some(&(x, y)) = tile_map.get(&robber_coordinate(game)) {
        let channel = 2 * num_players + 5;
        stamp_tile(&mut data, channel, x, y, 1.0);
    }

    for (resource, node_ids) in &game.map.port_nodes {
        let channel_delta = match resource {
            Some(res) => resource_index(*res),
            None => 5,
        };
        let channel = 2 * num_players + 5 + 1 + channel_delta;
        for node in node_ids {
            if let Some(&(x, y)) = node_map.get(node) {
                set_value(&mut data, channel, x, y, 1.0);
            }
        }
    }

    BoardTensor {
        width: WIDTH,
        height: HEIGHT,
        channels,
        data,
    }
}

fn gather_player_features(
    game: &GameState,
    order: &[(usize, &PlayerState)],
    features: &mut BTreeMap<String, f64>,
) {
    let blocked_nodes = blocked_nodes(game);
    for (relative_idx, (player_idx, player)) in order.iter().enumerate() {
        if relative_idx == 0 {
            features.insert("P0_ACTUAL_VPS".to_string(), player.total_points() as f64);
        }

        let public_vps = player.public_points();
        features.insert(format!("P{relative_idx}_PUBLIC_VPS"), public_vps as f64);
        features.insert(
            format!("P{relative_idx}_HAS_ARMY"),
            bool_to_f32(player.has_largest_army),
        );
        features.insert(
            format!("P{relative_idx}_HAS_ROAD"),
            bool_to_f32(player.has_longest_road),
        );
        features.insert(
            format!("P{relative_idx}_ROADS_LEFT"),
            (MAX_ROADS - player.roads.len()) as f64,
        );
        features.insert(
            format!("P{relative_idx}_SETTLEMENTS_LEFT"),
            (MAX_SETTLEMENTS - player.settlements.len()) as f64,
        );
        features.insert(
            format!("P{relative_idx}_CITIES_LEFT"),
            (MAX_CITIES - player.cities.len()) as f64,
        );
        features.insert(
            format!("P{relative_idx}_HAS_ROLLED"),
            bool_to_f32(player.has_rolled),
        );
        let longest = longest_road_length(game, *player_idx, &blocked_nodes);
        features.insert(
            format!("P{relative_idx}_LONGEST_ROAD_LENGTH"),
            longest as f64,
        );
    }
}

fn gather_resource_hand_features(
    order: &[(usize, &PlayerState)],
    features: &mut BTreeMap<String, f64>,
) {
    if order.is_empty() {
        return;
    }
    let perspective = order[0].1;

    for resource in Resource::ALL {
        let count = perspective.resources.get(resource);
        features.insert(format!("P0_{:?}_IN_HAND", resource), count as f64);
    }
    for card in DevelopmentCard::ALL {
        let count = perspective
            .dev_cards
            .iter()
            .chain(perspective.fresh_dev_cards.iter())
            .filter(|&&c| c == card)
            .count();
        features.insert(format!("P0_{:?}_IN_HAND", card), count as f64);
        for (relative_idx, (_, player)) in order.iter().enumerate() {
            if card == DevelopmentCard::VictoryPoint {
                continue;
            }
            let played = player.played_dev_cards.get(&card).copied().unwrap_or(0);
            features.insert(format!("P{relative_idx}_{:?}_PLAYED", card), played as f64);
        }
    }
    features.insert(
        "P0_HAS_PLAYED_DEVELOPMENT_CARD_IN_TURN".to_string(),
        bool_to_f32(perspective.has_played_dev_card_this_turn),
    );
    for (relative_idx, (_, player)) in order.iter().enumerate() {
        features.insert(
            format!("P{relative_idx}_NUM_RESOURCES_IN_HAND"),
            player.resources.total() as f64,
        );
        let dev_total = player.dev_cards.len() + player.fresh_dev_cards.len();
        features.insert(
            format!("P{relative_idx}_NUM_DEVS_IN_HAND"),
            dev_total as f64,
        );
    }
}

fn gather_tile_features(game: &GameState, features: &mut BTreeMap<String, f64>) {
    for (tile_id, tile) in &game.map.tiles_by_id {
        for resource in Resource::ALL {
            let value = bool_to_f32(tile.resource == Some(resource));
            features.insert(format!("TILE{tile_id}_IS_{resource:?}"), value);
        }
        features.insert(
            format!("TILE{tile_id}_IS_DESERT"),
            bool_to_f32(tile.resource.is_none()),
        );
        let proba = tile.number.map(number_probability).unwrap_or(0.0) as f64;
        features.insert(format!("TILE{tile_id}_PROBA"), proba);
        features.insert(
            format!("TILE{tile_id}_HAS_ROBBER"),
            bool_to_f32(tile.id == game.robber_tile),
        );
    }
}

fn gather_port_features(game: &GameState, features: &mut BTreeMap<String, f64>) {
    for (port_id, port) in &game.map.ports_by_id {
        for resource in Resource::ALL {
            features.insert(
                format!("PORT{port_id}_IS_{resource:?}"),
                bool_to_f32(port.resource == Some(resource)),
            );
        }
        features.insert(
            format!("PORT{port_id}_IS_THREE_TO_ONE"),
            bool_to_f32(port.resource.is_none()),
        );
    }
}

fn gather_graph_features(
    game: &GameState,
    order: &[(usize, &PlayerState)],
    features: &mut BTreeMap<String, f64>,
) {
    let nodes: BTreeSet<_> = game.map.land_nodes.iter().copied().collect();
    for (relative_idx, (_player_idx, player)) in order.iter().enumerate() {
        for node in &nodes {
            let settlement = player.settlements.contains(node);
            let city = player.cities.contains(node);
            features.insert(
                format!("NODE{node}_P{relative_idx}_SETTLEMENT"),
                bool_to_f32(settlement),
            );
            features.insert(
                format!("NODE{node}_P{relative_idx}_CITY"),
                bool_to_f32(city),
            );
        }

        for edge in all_edges(game) {
            let owned = player.roads.contains(&edge) || player.roads.contains(&(edge.1, edge.0));
            features.insert(
                format!("EDGE({},{})_P{relative_idx}_ROAD", edge.0, edge.1),
                bool_to_f32(owned),
            );
        }
    }
}

fn gather_game_features(game: &GameState, features: &mut BTreeMap<String, f64>) {
    features.insert(
        "BANK_DEV_CARDS".to_string(),
        game.bank.development_deck_len() as f64,
    );
    features.insert(
        "IS_MOVING_ROBBER".to_string(),
        bool_to_f32(matches!(game.pending_prompt, ActionPrompt::MoveRobber)),
    );
    features.insert(
        "IS_DISCARDING".to_string(),
        bool_to_f32(matches!(game.pending_prompt, ActionPrompt::Discard)),
    );
    for (resource, count) in game.bank.resources().iter() {
        features.insert(format!("BANK_{resource:?}"), count as f64);
    }
}

fn iter_players<'a>(game: &'a GameState, perspective: usize) -> Vec<(usize, &'a PlayerState)> {
    let mut result = Vec::with_capacity(game.players.len());
    for offset in 0..game.players.len() {
        let idx = (perspective + offset) % game.players.len();
        result.push((idx, &game.players[idx]));
    }
    result
}

fn bool_to_f32(value: bool) -> f64 {
    if value { 1.0 } else { 0.0 }
}

fn all_edges(game: &GameState) -> Vec<EdgeId> {
    let mut edges = BTreeSet::new();
    for edge_list in game.map.node_edges.values() {
        for edge in edge_list {
            edges.insert(normalize_edge(*edge));
        }
    }
    edges.into_iter().collect()
}

fn normalize_edge(edge: EdgeId) -> EdgeId {
    if edge.0 <= edge.1 {
        edge
    } else {
        (edge.1, edge.0)
    }
}

fn longest_road_length(
    game: &GameState,
    player_idx: usize,
    blocked_nodes: &HashSet<NodeId>,
) -> usize {
    let player = &game.players[player_idx];
    if player.roads.is_empty() {
        return 0;
    }
    let mut best = 0;
    for &(a, b) in &player.roads {
        best = best.max(longest_from_node(
            game,
            player_idx,
            a,
            blocked_nodes,
            &mut HashSet::new(),
        ));
        best = best.max(longest_from_node(
            game,
            player_idx,
            b,
            blocked_nodes,
            &mut HashSet::new(),
        ));
    }
    best
}

fn longest_from_node(
    game: &GameState,
    player_idx: usize,
    start: NodeId,
    blocked_nodes: &HashSet<NodeId>,
    visited_edges: &mut HashSet<EdgeId>,
) -> usize {
    let mut best = 0;
    if let Some(neighbors) = game.map.node_neighbors.get(&start) {
        for &neighbor in neighbors {
            if blocked_nodes.contains(&neighbor) && !owns_node(game, player_idx, neighbor) {
                continue;
            }
            let edge = normalize_edge((start, neighbor));
            if !game.players[player_idx].roads.contains(&edge)
                && !game.players[player_idx].roads.contains(&(edge.1, edge.0))
            {
                continue;
            }
            if visited_edges.contains(&edge) {
                continue;
            }
            visited_edges.insert(edge);
            let depth =
                1 + longest_from_node(game, player_idx, neighbor, blocked_nodes, visited_edges);
            visited_edges.remove(&edge);
            if depth > best {
                best = depth;
            }
        }
    }
    best
}

fn owns_node(game: &GameState, player_idx: usize, node: NodeId) -> bool {
    match game.node_occupancy.get(&node) {
        Some(Structure::Settlement { player }) | Some(Structure::City { player }) => {
            *player == player_idx
        }
        None => false,
    }
}

fn blocked_nodes(game: &GameState) -> HashSet<NodeId> {
    game.node_occupancy.keys().copied().collect()
}

type BoardMaps = (
    HashMap<NodeId, (usize, usize)>,
    HashMap<EdgeId, (usize, usize)>,
);

fn board_maps() -> &'static BoardMaps {
    static MAPS: Lazy<BoardMaps> = Lazy::new(|| {
        let graph = base_graph();
        let mut node_map: HashMap<NodeId, (usize, usize)> = HashMap::new();
        let mut edge_map: HashMap<EdgeId, (usize, usize)> = HashMap::new();
        let mut paths = Vec::new();
        for &(start, end) in PAIRS {
            let path = shortest_path(&graph, start, end).expect("path exists");
            paths.push(path);
        }
        for (i, path) in paths.iter().enumerate() {
            for (j, &node) in path.iter().enumerate() {
                node_map.insert(node, (2 * j, 2 * i));

                let node_has_down_edge = (i + j) % 2 == 0;
                if node_has_down_edge && i + 1 < paths.len() {
                    let next_path = &paths[i + 1];
                    if j < next_path.len() {
                        let neighbor = next_path[j];
                        edge_map.insert((node, neighbor), (2 * j, 2 * i + 1));
                        edge_map.insert((neighbor, node), (2 * j, 2 * i + 1));
                    }
                }

                if j + 1 < path.len() {
                    let neighbor = path[j + 1];
                    edge_map.insert((node, neighbor), (2 * j + 1, 2 * i));
                    edge_map.insert((neighbor, node), (2 * j + 1, 2 * i));
                }
            }
        }
        (node_map, edge_map)
    });
    &MAPS
}

fn node_position_map() -> &'static HashMap<NodeId, (usize, usize)> {
    &board_maps().0
}

fn edge_position_map() -> &'static HashMap<EdgeId, (usize, usize)> {
    &board_maps().1
}

fn tile_coordinate_map() -> &'static HashMap<CubeCoord, (usize, usize)> {
    static MAP: Lazy<HashMap<CubeCoord, (usize, usize)>> = Lazy::new(|| {
        let mut map = HashMap::new();
        let width_step = 4;
        let height_step = 2;
        for i in 0..(HEIGHT / height_step) {
            for j in 0..(WIDTH / width_step) {
                let offset_x = -2 + j as i32;
                let offset_y = -2 + i as i32;
                let cube = offset_to_cube((offset_x, offset_y));
                let maybe_odd_offset = (i % 2) * 2;
                map.insert(cube, (height_step * i, width_step * j + maybe_odd_offset));
            }
        }
        map
    });
    &MAP
}

fn robber_coordinate(game: &GameState) -> CubeCoord {
    for (coord, tile) in &game.map.land_tiles {
        if tile.id == game.robber_tile {
            return *coord;
        }
    }
    CubeCoord::default()
}

fn base_graph() -> &'static HashMap<NodeId, Vec<NodeId>> {
    static GRAPH: Lazy<HashMap<NodeId, Vec<NodeId>>> = Lazy::new(|| {
        let base = CatanMap::build(crate::board::MapType::Base);
        let mut graph: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();
        for tile in base.tiles.values() {
            let edges = match tile {
                crate::board::Tile::Land(t) => &t.edges,
                crate::board::Tile::Port(t) => &t.edges,
                crate::board::Tile::Water(t) => &t.edges,
            };
            for &(a, b) in edges.values() {
                graph.entry(a).or_default().insert(b);
                graph.entry(b).or_default().insert(a);
            }
        }
        graph
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().collect()))
            .collect()
    });
    &GRAPH
}

fn shortest_path(
    graph: &HashMap<NodeId, Vec<NodeId>>,
    start: NodeId,
    end: NodeId,
) -> Option<Vec<NodeId>> {
    let mut queue = VecDeque::new();
    let mut parents: HashMap<NodeId, NodeId> = HashMap::new();
    let mut visited = HashSet::new();
    queue.push_back(start);
    visited.insert(start);

    while let Some(node) = queue.pop_front() {
        if node == end {
            let mut path = vec![node];
            let mut current = node;
            while let Some(&parent) = parents.get(&current) {
                path.push(parent);
                current = parent;
            }
            path.reverse();
            return Some(path);
        }
        if let Some(neighbors) = graph.get(&node) {
            for &neighbor in neighbors {
                if visited.insert(neighbor) {
                    parents.insert(neighbor, node);
                    queue.push_back(neighbor);
                }
            }
        }
    }
    None
}

fn set_value(data: &mut [f32], channel: usize, x: usize, y: usize, value: f32) {
    if x >= WIDTH || y >= HEIGHT {
        return;
    }
    let channels = data.len() / (WIDTH * HEIGHT);
    if channel >= channels {
        return;
    }
    let idx = (y * WIDTH + x) * channels + channel;
    if idx < data.len() {
        data[idx] = value;
    }
}

fn stamp_tile(data: &mut [f32], channel: usize, x: usize, y: usize, value: f32) {
    for dx in [0, 2, 4] {
        for dy in [0, 2] {
            let nx = x + dx;
            let ny = y + dy;
            if nx < WIDTH && ny < HEIGHT {
                set_value(data, channel, nx, ny, value);
            }
        }
    }
}

fn number_probability(number: u8) -> f32 {
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

fn resource_index(resource: Resource) -> usize {
    match resource {
        Resource::Wood => 0,
        Resource::Brick => 1,
        Resource::Sheep => 2,
        Resource::Wheat => 3,
        Resource::Ore => 4,
    }
}
