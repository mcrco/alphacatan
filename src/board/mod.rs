use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::str::FromStr;

use once_cell::sync::Lazy;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use crate::coords::{CubeCoord, Direction, UNIT_VECTORS, add};
use crate::types::{EdgeRef, NodeRef, Resource};

mod node_ids;

pub type NodeId = u16;
pub type EdgeId = (NodeId, NodeId);

type NodeMap = HashMap<NodeRef, NodeId>;
type EdgeMap = HashMap<EdgeRef, EdgeId>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandTile {
    pub id: u16,
    pub resource: Option<Resource>,
    pub number: Option<u8>,
    pub nodes: NodeMap,
    pub edges: EdgeMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Port {
    pub id: u16,
    pub resource: Option<Resource>,
    pub direction: Direction,
    pub nodes: NodeMap,
    pub edges: EdgeMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Water {
    pub nodes: NodeMap,
    pub edges: EdgeMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Tile {
    Land(LandTile),
    Port(Port),
    Water(Water),
}

impl Tile {
    fn nodes(&self) -> &NodeMap {
        match self {
            Tile::Land(tile) => &tile.nodes,
            Tile::Port(port) => &port.nodes,
            Tile::Water(water) => &water.nodes,
        }
    }

    fn edges(&self) -> &EdgeMap {
        match self {
            Tile::Land(tile) => &tile.edges,
            Tile::Port(port) => &port.edges,
            Tile::Water(water) => &water.edges,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TileTemplate {
    Land,
    Water,
    Port(Direction),
}

#[derive(Debug, Clone)]
pub struct MapTemplate {
    pub numbers: Vec<u8>,
    pub port_resources: Vec<Option<Resource>>,
    pub tile_resources: Vec<Option<Resource>>,
    pub topology: Vec<(CubeCoord, TileTemplate)>,
    pub node_lookup: Option<&'static HashMap<(CubeCoord, NodeRef), NodeId>>,
}

impl MapTemplate {
    pub fn base() -> &'static MapTemplate {
        &BASE_TEMPLATE
    }

    pub fn mini() -> &'static MapTemplate {
        &MINI_TEMPLATE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MapType {
    Base,
    Tournament,
    Mini,
}

impl Default for MapType {
    fn default() -> Self {
        MapType::Base
    }
}

impl fmt::Display for MapType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            MapType::Base => "BASE",
            MapType::Tournament => "TOURNAMENT",
            MapType::Mini => "MINI",
        };
        write!(f, "{label}")
    }
}

impl FromStr for MapType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "base" => Ok(MapType::Base),
            "tournament" => Ok(MapType::Tournament),
            "mini" => Ok(MapType::Mini),
            _ => Err(format!("unknown map type: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MapShuffleOverrides<'a> {
    pub numbers: Option<&'a [u8]>,
    pub port_resources: Option<&'a [Option<Resource>]>,
    pub tile_resources: Option<&'a [Option<Resource>]>,
}

#[derive(Debug, Clone)]
pub struct CatanMap {
    pub tiles: HashMap<CubeCoord, Tile>,
    pub land_tiles: HashMap<CubeCoord, LandTile>,
    pub port_nodes: HashMap<Option<Resource>, HashSet<NodeId>>,
    pub land_nodes: HashSet<NodeId>,
    pub adjacent_tiles: HashMap<NodeId, Vec<u16>>,
    pub node_neighbors: HashMap<NodeId, HashSet<NodeId>>,
    pub node_edges: HashMap<NodeId, Vec<EdgeId>>,
    pub node_production: HashMap<NodeId, BTreeMap<Resource, f32>>,
    pub tiles_by_id: HashMap<u16, LandTile>,
    pub ports_by_id: HashMap<u16, Port>,
}

impl CatanMap {
    pub fn from_template(template: &MapTemplate, overrides: MapShuffleOverrides<'_>) -> Self {
        let mut rng = thread_rng();
        Self::from_template_with_rng(template, overrides, &mut rng)
    }

    pub fn from_template_with_rng(
        template: &MapTemplate,
        overrides: MapShuffleOverrides<'_>,
        rng: &mut impl rand::Rng,
    ) -> Self {
        let tiles = initialize_tiles(template, overrides, rng);
        Self::from_tiles(tiles)
    }

    pub fn from_tiles(tiles: HashMap<CubeCoord, Tile>) -> Self {
        let land_tiles: HashMap<CubeCoord, LandTile> = tiles
            .iter()
            .filter_map(|(coord, tile)| match tile {
                Tile::Land(land) => Some((*coord, land.clone())),
                _ => None,
            })
            .collect();

        let tiles_by_id: HashMap<u16, LandTile> = land_tiles
            .values()
            .map(|tile| (tile.id, tile.clone()))
            .collect();

        let mut port_nodes: HashMap<Option<Resource>, HashSet<NodeId>> = HashMap::new();
        for tile in tiles.values() {
            if let Tile::Port(port) = tile {
                let (first_ref, second_ref) = PORT_DIRECTION_TO_NODE_REFS
                    .get(&port.direction)
                    .expect("missing port");
                port_nodes
                    .entry(port.resource)
                    .or_default()
                    .insert(*port.nodes.get(first_ref).expect("node missing"));
                port_nodes
                    .entry(port.resource)
                    .or_default()
                    .insert(*port.nodes.get(second_ref).expect("node missing"));
            }
        }

        let land_nodes: HashSet<NodeId> = land_tiles
            .values()
            .flat_map(|tile| tile.nodes.values().copied())
            .collect();

        let mut adjacent_tiles: HashMap<NodeId, Vec<u16>> = HashMap::new();
        for tile in land_tiles.values() {
            for node_id in tile.nodes.values() {
                adjacent_tiles.entry(*node_id).or_default().push(tile.id);
            }
        }

        let mut node_neighbors: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();
        let mut node_edges: HashMap<NodeId, Vec<EdgeId>> = HashMap::new();
        for tile in tiles.values() {
            for edge in tile.edges().values() {
                let (a, b) = *edge;
                node_neighbors.entry(a).or_default().insert(b);
                node_neighbors.entry(b).or_default().insert(a);
                node_edges.entry(a).or_default().push(*edge);
                node_edges.entry(b).or_default().push(*edge);
            }
        }

        let node_production: HashMap<NodeId, BTreeMap<Resource, f32>> = adjacent_tiles
            .iter()
            .map(|(node_id, tile_ids)| {
                let mut production: BTreeMap<Resource, f32> = BTreeMap::new();
                for tile_id in tile_ids {
                    if let Some(tile) = tiles_by_id.get(tile_id) {
                        if let (Some(resource), Some(number)) = (tile.resource, tile.number) {
                            let entry = production.entry(resource).or_default();
                            *entry += number_probability(number);
                        }
                    }
                }
                (*node_id, production)
            })
            .collect();
        let ports_by_id = tiles
            .values()
            .filter_map(|tile| match tile {
                Tile::Port(port) => Some((port.id, port.clone())),
                _ => None,
            })
            .collect();

        Self {
            tiles,
            land_tiles,
            port_nodes,
            land_nodes,
            adjacent_tiles,
            node_production,
            node_edges,
            node_neighbors,
            tiles_by_id,
            ports_by_id,
        }
    }

    pub fn build(map_type: MapType) -> Self {
        let mut rng = thread_rng();
        Self::build_with_rng(map_type, &mut rng)
    }

    pub fn build_with_rng(map_type: MapType, rng: &mut impl rand::Rng) -> Self {
        match map_type {
            MapType::Base => {
                CatanMap::from_template_with_rng(MapTemplate::base(), MapShuffleOverrides::default(), rng)
            }
            MapType::Mini => {
                CatanMap::from_template_with_rng(MapTemplate::mini(), MapShuffleOverrides::default(), rng)
            }
            MapType::Tournament => build_tournament_map(),
        }
    }
}

fn build_tournament_map() -> CatanMap {
    static TOURNAMENT_NUMBERS: Lazy<Vec<u8>> =
        Lazy::new(|| vec![10, 8, 3, 6, 2, 5, 10, 8, 4, 11, 12, 9, 5, 4, 9, 11, 3, 6]);
    static TOURNAMENT_PORTS: Lazy<Vec<Option<Resource>>> = Lazy::new(|| {
        vec![
            None,
            Some(Resource::Sheep),
            None,
            Some(Resource::Ore),
            Some(Resource::Wheat),
            None,
            Some(Resource::Wood),
            Some(Resource::Brick),
            None,
        ]
    });
    static TOURNAMENT_TILES: Lazy<Vec<Option<Resource>>> = Lazy::new(|| {
        vec![
            None,
            Some(Resource::Wood),
            Some(Resource::Sheep),
            Some(Resource::Sheep),
            Some(Resource::Wood),
            Some(Resource::Wheat),
            Some(Resource::Wood),
            Some(Resource::Wheat),
            Some(Resource::Brick),
            Some(Resource::Sheep),
            Some(Resource::Brick),
            Some(Resource::Sheep),
            Some(Resource::Wheat),
            Some(Resource::Wheat),
            Some(Resource::Ore),
            Some(Resource::Brick),
            Some(Resource::Ore),
            Some(Resource::Wood),
            Some(Resource::Ore),
            None,
        ]
    });

    CatanMap::from_template(
        MapTemplate::base(),
        MapShuffleOverrides {
            numbers: Some(&TOURNAMENT_NUMBERS),
            port_resources: Some(&TOURNAMENT_PORTS),
            tile_resources: Some(&TOURNAMENT_TILES),
        },
    )
}

fn initialize_tiles(
    template: &MapTemplate,
    overrides: MapShuffleOverrides<'_>,
    rng: &mut impl rand::Rng,
) -> HashMap<CubeCoord, Tile> {
    let mut numbers = overrides
        .numbers
        .map(|slice| slice.to_vec())
        .unwrap_or_else(|| template.numbers.clone());
    if overrides.numbers.is_none() {
        numbers.shuffle(rng);
    }

    let mut port_resources = overrides
        .port_resources
        .map(|slice| slice.to_vec())
        .unwrap_or_else(|| template.port_resources.clone());
    if overrides.port_resources.is_none() {
        port_resources.shuffle(rng);
    }

    let mut tile_resources = overrides
        .tile_resources
        .map(|slice| slice.to_vec())
        .unwrap_or_else(|| template.tile_resources.clone());
    if overrides.tile_resources.is_none() {
        tile_resources.shuffle(rng);
    }

    let mut tiles: HashMap<CubeCoord, Tile> = HashMap::new();
    let mut node_autoinc: NodeId = 0;
    let mut land_autoinc: u16 = 0;
    let mut port_autoinc: u16 = 0;

    for (coord, template_kind) in &template.topology {
        let (nodes, edges, next_autoinc) =
            get_nodes_and_edges(&tiles, *coord, node_autoinc, template.node_lookup);
        node_autoinc = next_autoinc;

        match template_kind {
            TileTemplate::Land => {
                let resource = tile_resources.pop().expect("not enough tile resources");
                if let Some(res) = resource {
                    let number = numbers.pop().expect("not enough numbers");
                    let tile = LandTile {
                        id: land_autoinc,
                        resource: Some(res),
                        number: Some(number),
                        nodes,
                        edges,
                    };
                    tiles.insert(*coord, Tile::Land(tile));
                } else {
                    let tile = LandTile {
                        id: land_autoinc,
                        resource: None,
                        number: None,
                        nodes,
                        edges,
                    };
                    tiles.insert(*coord, Tile::Land(tile));
                }
                land_autoinc += 1;
            }
            TileTemplate::Water => {
                tiles.insert(*coord, Tile::Water(Water { nodes, edges }));
            }
            TileTemplate::Port(direction) => {
                let resource = port_resources.pop().expect("not enough port resources");
                let port = Port {
                    id: port_autoinc,
                    resource,
                    direction: *direction,
                    nodes,
                    edges,
                };
                tiles.insert(*coord, Tile::Port(port));
                port_autoinc += 1;
            }
        }
    }

    tiles
}

fn get_nodes_and_edges(
    tiles: &HashMap<CubeCoord, Tile>,
    coordinate: CubeCoord,
    mut node_autoinc: NodeId,
    node_lookup: Option<&HashMap<(CubeCoord, NodeRef), NodeId>>,
) -> (NodeMap, EdgeMap, NodeId) {
    let mut nodes: HashMap<NodeRef, Option<NodeId>> = NodeRef::iter().map(|n| (n, None)).collect();
    let mut edges: HashMap<EdgeRef, Option<EdgeId>> = EdgeRef::iter().map(|e| (e, None)).collect();

    for direction in Direction::iter() {
        let offset = UNIT_VECTORS
            .get(&direction)
            .copied()
            .expect("unit vector missing");
        let neighbor_coord = add(coordinate, offset);
        if let Some(neighbor) = tiles.get(&neighbor_coord) {
            match direction {
                Direction::East => {
                    nodes.insert(
                        NodeRef::NorthEast,
                        neighbor.nodes().get(&NodeRef::NorthWest).copied(),
                    );
                    nodes.insert(
                        NodeRef::SouthEast,
                        neighbor.nodes().get(&NodeRef::SouthWest).copied(),
                    );
                    edges.insert(EdgeRef::East, neighbor.edges().get(&EdgeRef::West).copied());
                }
                Direction::SouthEast => {
                    nodes.insert(
                        NodeRef::South,
                        neighbor.nodes().get(&NodeRef::NorthWest).copied(),
                    );
                    nodes.insert(
                        NodeRef::SouthEast,
                        neighbor.nodes().get(&NodeRef::North).copied(),
                    );
                    edges.insert(
                        EdgeRef::SouthEast,
                        neighbor.edges().get(&EdgeRef::NorthWest).copied(),
                    );
                }
                Direction::SouthWest => {
                    nodes.insert(
                        NodeRef::South,
                        neighbor.nodes().get(&NodeRef::NorthEast).copied(),
                    );
                    nodes.insert(
                        NodeRef::SouthWest,
                        neighbor.nodes().get(&NodeRef::North).copied(),
                    );
                    edges.insert(
                        EdgeRef::SouthWest,
                        neighbor.edges().get(&EdgeRef::NorthEast).copied(),
                    );
                }
                Direction::West => {
                    nodes.insert(
                        NodeRef::NorthWest,
                        neighbor.nodes().get(&NodeRef::NorthEast).copied(),
                    );
                    nodes.insert(
                        NodeRef::SouthWest,
                        neighbor.nodes().get(&NodeRef::SouthEast).copied(),
                    );
                    edges.insert(EdgeRef::West, neighbor.edges().get(&EdgeRef::East).copied());
                }
                Direction::NorthWest => {
                    nodes.insert(
                        NodeRef::North,
                        neighbor.nodes().get(&NodeRef::SouthEast).copied(),
                    );
                    nodes.insert(
                        NodeRef::NorthWest,
                        neighbor.nodes().get(&NodeRef::South).copied(),
                    );
                    edges.insert(
                        EdgeRef::NorthWest,
                        neighbor.edges().get(&EdgeRef::SouthEast).copied(),
                    );
                }
                Direction::NorthEast => {
                    nodes.insert(
                        NodeRef::North,
                        neighbor.nodes().get(&NodeRef::SouthWest).copied(),
                    );
                    nodes.insert(
                        NodeRef::NorthEast,
                        neighbor.nodes().get(&NodeRef::South).copied(),
                    );
                    edges.insert(
                        EdgeRef::NorthEast,
                        neighbor.edges().get(&EdgeRef::SouthWest).copied(),
                    );
                }
            }
        }
    }

    for (node_ref, node_entry) in nodes.iter_mut() {
        if node_entry.is_none() {
            if let Some(lookup) = node_lookup {
                if let Some(id) = lookup.get(&(coordinate, *node_ref)) {
                    *node_entry = Some(*id);
                    continue;
                }
            }
            *node_entry = Some(node_autoinc);
            node_autoinc += 1;
        }
    }

    for (edge_ref, value) in edges.iter_mut() {
        if value.is_none() {
            let (a_ref, b_ref) = get_edge_nodes(*edge_ref);
            let a = nodes
                .get(&a_ref)
                .and_then(|x| *x)
                .expect("node missing during edge construction");
            let b = nodes
                .get(&b_ref)
                .and_then(|x| *x)
                .expect("node missing during edge construction");
            *value = Some((a, b));
        }
    }

    let finalized_nodes = nodes
        .into_iter()
        .map(|(k, v)| (k, v.expect("node missing")))
        .collect();
    let finalized_edges = edges
        .into_iter()
        .map(|(k, v)| (k, v.expect("edge missing")))
        .collect();

    (finalized_nodes, finalized_edges, node_autoinc)
}

fn get_edge_nodes(edge_ref: EdgeRef) -> (NodeRef, NodeRef) {
    match edge_ref {
        EdgeRef::East => (NodeRef::NorthEast, NodeRef::SouthEast),
        EdgeRef::SouthEast => (NodeRef::SouthEast, NodeRef::South),
        EdgeRef::SouthWest => (NodeRef::South, NodeRef::SouthWest),
        EdgeRef::West => (NodeRef::SouthWest, NodeRef::NorthWest),
        EdgeRef::NorthWest => (NodeRef::NorthWest, NodeRef::North),
        EdgeRef::NorthEast => (NodeRef::North, NodeRef::NorthEast),
    }
}

static PORT_DIRECTION_TO_NODE_REFS: Lazy<HashMap<Direction, (NodeRef, NodeRef)>> =
    Lazy::new(|| {
        HashMap::from([
            (Direction::West, (NodeRef::NorthWest, NodeRef::SouthWest)),
            (Direction::NorthWest, (NodeRef::North, NodeRef::NorthWest)),
            (Direction::NorthEast, (NodeRef::NorthEast, NodeRef::North)),
            (Direction::East, (NodeRef::SouthEast, NodeRef::NorthEast)),
            (Direction::SouthEast, (NodeRef::South, NodeRef::SouthEast)),
            (Direction::SouthWest, (NodeRef::SouthWest, NodeRef::South)),
        ])
    });

fn number_probability(number: u8) -> f32 {
    *DICE_PROBABILITIES.get(&number).unwrap_or(&0.0)
}

static DICE_PROBABILITIES: Lazy<HashMap<u8, f32>> = Lazy::new(|| {
    let mut probas: HashMap<u8, f32> = HashMap::new();
    for i in 1..=6 {
        for j in 1..=6 {
            let total = (i + j) as u8;
            *probas.entry(total).or_insert(0.0) += 1.0 / 36.0;
        }
    }
    probas
});

static BASE_TEMPLATE: Lazy<MapTemplate> = Lazy::new(|| MapTemplate {
    numbers: vec![2, 3, 3, 4, 4, 5, 5, 6, 6, 8, 8, 9, 9, 10, 10, 11, 11, 12],
    port_resources: vec![
        Some(Resource::Wood),
        Some(Resource::Brick),
        Some(Resource::Sheep),
        Some(Resource::Wheat),
        Some(Resource::Ore),
        None,
        None,
        None,
        None,
    ],
    tile_resources: vec![
        Some(Resource::Wood),
        Some(Resource::Wood),
        Some(Resource::Wood),
        Some(Resource::Wood),
        Some(Resource::Brick),
        Some(Resource::Brick),
        Some(Resource::Brick),
        Some(Resource::Sheep),
        Some(Resource::Sheep),
        Some(Resource::Sheep),
        Some(Resource::Sheep),
        Some(Resource::Wheat),
        Some(Resource::Wheat),
        Some(Resource::Wheat),
        Some(Resource::Wheat),
        Some(Resource::Ore),
        Some(Resource::Ore),
        Some(Resource::Ore),
        None,
    ],
    topology: base_topology(),
    node_lookup: Some(&node_ids::BASE_NODE_IDS),
});

static MINI_TEMPLATE: Lazy<MapTemplate> = Lazy::new(|| MapTemplate {
    numbers: vec![3, 4, 5, 6, 8, 9, 10],
    port_resources: vec![],
    tile_resources: vec![
        Some(Resource::Wood),
        None,
        Some(Resource::Brick),
        Some(Resource::Sheep),
        Some(Resource::Wheat),
        Some(Resource::Wheat),
        Some(Resource::Ore),
    ],
    topology: mini_topology(),
    node_lookup: Some(&node_ids::MINI_NODE_IDS),
});

fn base_topology() -> Vec<(CubeCoord, TileTemplate)> {
    use TileTemplate::*;
    vec![
        (CubeCoord::new(0, 0, 0), Land),
        (CubeCoord::new(1, -1, 0), Land),
        (CubeCoord::new(0, -1, 1), Land),
        (CubeCoord::new(-1, 0, 1), Land),
        (CubeCoord::new(-1, 1, 0), Land),
        (CubeCoord::new(0, 1, -1), Land),
        (CubeCoord::new(1, 0, -1), Land),
        (CubeCoord::new(2, -2, 0), Land),
        (CubeCoord::new(1, -2, 1), Land),
        (CubeCoord::new(0, -2, 2), Land),
        (CubeCoord::new(-1, -1, 2), Land),
        (CubeCoord::new(-2, 0, 2), Land),
        (CubeCoord::new(-2, 1, 1), Land),
        (CubeCoord::new(-2, 2, 0), Land),
        (CubeCoord::new(-1, 2, -1), Land),
        (CubeCoord::new(0, 2, -2), Land),
        (CubeCoord::new(1, 1, -2), Land),
        (CubeCoord::new(2, 0, -2), Land),
        (CubeCoord::new(2, -1, -1), Land),
        (CubeCoord::new(3, -3, 0), Port(Direction::West)),
        (CubeCoord::new(2, -3, 1), Water),
        (CubeCoord::new(1, -3, 2), Port(Direction::NorthWest)),
        (CubeCoord::new(0, -3, 3), Water),
        (CubeCoord::new(-1, -2, 3), Port(Direction::NorthWest)),
        (CubeCoord::new(-2, -1, 3), Water),
        (CubeCoord::new(-3, 0, 3), Port(Direction::NorthEast)),
        (CubeCoord::new(-3, 1, 2), Water),
        (CubeCoord::new(-3, 2, 1), Port(Direction::East)),
        (CubeCoord::new(-3, 3, 0), Water),
        (CubeCoord::new(-2, 3, -1), Port(Direction::East)),
        (CubeCoord::new(-1, 3, -2), Water),
        (CubeCoord::new(0, 3, -3), Port(Direction::SouthEast)),
        (CubeCoord::new(1, 2, -3), Water),
        (CubeCoord::new(2, 1, -3), Port(Direction::SouthWest)),
        (CubeCoord::new(3, 0, -3), Water),
        (CubeCoord::new(3, -1, -2), Port(Direction::SouthWest)),
        (CubeCoord::new(3, -2, -1), Water),
    ]
}

fn mini_topology() -> Vec<(CubeCoord, TileTemplate)> {
    use TileTemplate::*;
    vec![
        (CubeCoord::new(0, 0, 0), Land),
        (CubeCoord::new(1, -1, 0), Land),
        (CubeCoord::new(0, -1, 1), Land),
        (CubeCoord::new(-1, 0, 1), Land),
        (CubeCoord::new(-1, 1, 0), Land),
        (CubeCoord::new(0, 1, -1), Land),
        (CubeCoord::new(1, 0, -1), Land),
        (CubeCoord::new(2, -2, 0), Water),
        (CubeCoord::new(1, -2, 1), Water),
        (CubeCoord::new(0, -2, 2), Water),
        (CubeCoord::new(-1, -1, 2), Water),
        (CubeCoord::new(-2, 0, 2), Water),
        (CubeCoord::new(-2, 1, 1), Water),
        (CubeCoord::new(-2, 2, 0), Water),
        (CubeCoord::new(-1, 2, -1), Water),
        (CubeCoord::new(0, 2, -2), Water),
        (CubeCoord::new(1, 1, -2), Water),
        (CubeCoord::new(2, 0, -2), Water),
        (CubeCoord::new(2, -1, -1), Water),
    ]
}
