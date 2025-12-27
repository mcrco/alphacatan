use catanatron_rs::board::{CatanMap, MapType};
use serde_json;
use std::collections::HashMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let map_type = if args.len() > 1 && args[1] == "mini" {
        MapType::Mini
    } else {
        MapType::Base
    };

    let map = CatanMap::build(map_type);

    // Build a map from (x, y, z, node_ref) -> node_id
    let mut node_map: HashMap<(i32, i32, i32, &str), u16> = HashMap::new();

    for (coord, tile) in &map.tiles {
        use catanatron_rs::types::NodeRef;
        let node_iter = match tile {
            catanatron_rs::board::Tile::Land(t) => &t.nodes,
            catanatron_rs::board::Tile::Port(t) => &t.nodes,
            catanatron_rs::board::Tile::Water(t) => &t.nodes,
        };
        for (node_ref, node_id) in node_iter.iter() {
            let node_ref_str = match node_ref {
                NodeRef::North => "North",
                NodeRef::NorthEast => "NorthEast",
                NodeRef::SouthEast => "SouthEast",
                NodeRef::South => "South",
                NodeRef::SouthWest => "SouthWest",
                NodeRef::NorthWest => "NorthWest",
            };
            node_map.insert((coord.x, coord.y, coord.z, node_ref_str), *node_id);
        }
    }

    // Convert to JSON-serializable format
    let mut json_data: Vec<((i32, i32, i32), String, u16)> = node_map
        .iter()
        .map(|((x, y, z, nr), id)| ((*x, *y, *z), nr.to_string(), *id))
        .collect();
    json_data.sort();

    let output = serde_json::to_string_pretty(&json_data).unwrap();
    println!("{}", output);
}
