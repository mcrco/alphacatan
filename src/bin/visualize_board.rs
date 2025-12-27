use std::collections::HashMap;

use catanatron_rs::board::{CatanMap, MapType, Tile};
use catanatron_rs::coords::{CubeCoord, Direction};
use catanatron_rs::types::{NodeRef, Resource};
use plotters::prelude::*;

const LAND_COLOR: RGBColor = RGBColor(0x8B, 0x45, 0x13); // SaddleBrown-ish
const WATER_COLOR: RGBColor = RGBColor(0x41, 0x69, 0xE1); // RoyalBlue
const PORT_COLOR: RGBColor = RGBColor(0xFF, 0xD7, 0x00); // Gold

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Generating board visualizations (Rust backend)...");

    render_map(MapType::Base, "catan_base_map.png", 48.0)?;
    render_map(MapType::Mini, "catan_mini_map.png", 56.0)?;

    println!("Done.");
    Ok(())
}

fn render_map(
    map_type: MapType,
    filename: &str,
    hex_size: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let map = CatanMap::build(map_type);

    // Gather geometry
    let mut centers: Vec<(f64, f64, TileKind, Option<String>)> = Vec::new();
    let mut node_positions: HashMap<u16, Vec<(f64, f64)>> = HashMap::new();
    let mut all_points: Vec<(f64, f64)> = Vec::new();

    for (coord, tile) in &map.tiles {
        let (cx, cy) = cube_to_pixel(*coord, hex_size);
        let (kind, label) = match tile {
            Tile::Land(land) => (TileKind::Land, Some(land.id.to_string())),
            Tile::Water(_) => (TileKind::Water, None),
            Tile::Port(port) => (TileKind::Port, Some(format!("P{}", port.id))),
        };
        centers.push((cx, cy, kind, label));

        // collect corners for bounding box
        for (x, y) in hexagon_corners((cx, cy), hex_size) {
            all_points.push((x, y));
        }

        // collect node positions for labels (average if shared)
        let node_iter = match tile {
            Tile::Land(t) => &t.nodes,
            Tile::Port(t) => &t.nodes,
            Tile::Water(t) => &t.nodes,
        };
        for (node_ref, node_id) in node_iter.iter() {
            let pos = node_position((cx, cy), hex_size, *node_ref);
            node_positions.entry(*node_id).or_default().push(pos);
        }
    }

    // Compute bounding box
    let (min_x, max_x, min_y, max_y) = bounds(&all_points)?;
    let padding = hex_size * 2.0;
    let width = ((max_x - min_x) + 2.0 * padding).ceil() as u32;
    let height = ((max_y - min_y) + 2.0 * padding).ceil() as u32;

    let backend = BitMapBackend::new(filename, (width, height));
    let root = backend.into_drawing_area();
    root.fill(&WHITE)?;

    let to_canvas = |(x, y): (f64, f64)| -> (i32, i32) {
        let tx = x - min_x + padding;
        let ty = y - min_y + padding; // keep orientation consistent with Python rendering
        (tx.round() as i32, ty.round() as i32)
    };

    // Draw tiles
    for (cx, cy, kind, label) in &centers {
        let corners: Vec<(i32, i32)> = hexagon_corners((*cx, *cy), hex_size)
            .into_iter()
            .map(to_canvas)
            .collect();
        let color = match kind {
            TileKind::Land => LAND_COLOR,
            TileKind::Water => WATER_COLOR,
            TileKind::Port => PORT_COLOR,
        };
        let polygon = Polygon::new(corners, ShapeStyle::from(&color).filled());
        root.draw(&polygon)?;

        if let Some(text) = label {
            let (tx, ty) = to_canvas((*cx, *cy));
            root.draw(&Text::new(
                text.clone(),
                (tx, ty),
                ("sans-serif", 14).into_font().color(&BLACK),
            ))?;
        }
    }

    // Draw node circles + labels
    let mut node_centers: HashMap<u16, (f64, f64)> = HashMap::new();
    for (node_id, positions) in node_positions {
        let center = average(&positions);
        node_centers.insert(node_id, center);
    }

    let mut port_infos: Vec<PortInfo> = map
        .ports_by_id
        .values()
        .filter_map(|port| {
            let (a_ref, b_ref) = dock_node_refs(port.direction);
            let a = port.nodes.get(&a_ref)?;
            let b = port.nodes.get(&b_ref)?;
            Some(PortInfo {
                id: port.id,
                resource: port.resource,
                nodes: vec![*a, *b],
            })
        })
        .collect();
    port_infos.sort_by_key(|info| info.id);

    let mut port_node_labels: HashMap<u16, String> = HashMap::new();
    for port in map.ports_by_id.values() {
        let (a_ref, b_ref) = dock_node_refs(port.direction);
        if let (Some(a), Some(b)) = (port.nodes.get(&a_ref), port.nodes.get(&b_ref)) {
            port_node_labels.insert(*a, format!("P{}", port.id));
            port_node_labels.insert(*b, format!("P{}", port.id));
        }
    }

    for (node_id, (avg_x, avg_y)) in &node_centers {
        let (px, py) = to_canvas((*avg_x, *avg_y));
        let radius = (hex_size * 0.18).max(4.0) as i32;

        root.draw(&Circle::new(
            (px, py),
            radius,
            ShapeStyle::from(&WHITE).filled().stroke_width(1),
        ))?;
        root.draw(&Text::new(
            format!("{}", node_id),
            (px, py),
            ("sans-serif", 12).into_font().color(&BLACK),
        ))?;
        if let Some(text) = port_node_labels.get(node_id) {
            root.draw(&Text::new(
                text.clone(),
                (px, py + 14),
                ("sans-serif", 10).into_font().color(&BLACK),
            ))?;
        }
    }

    // Annotate ports with node ids and trade info
    for info in &port_infos {
        let nodes_text = info
            .nodes
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let trade_label = port_trade_label(info.resource);
        println!(
            "Port {:02} {:<8} nodes [{}]",
            info.id, trade_label, nodes_text
        );

        let node_points: Vec<(f64, f64)> = info
            .nodes
            .iter()
            .filter_map(|id| node_centers.get(id).copied())
            .collect();
        if node_points.is_empty() {
            continue;
        }
        let (avg_x, avg_y) = average(&node_points);
        let (px, py) = to_canvas((avg_x, avg_y));
        root.draw(&Text::new(
            trade_label,
            (px, py - 16),
            ("sans-serif", 12).into_font().color(&BLACK),
        ))?;
    }

    Ok(())
}

fn cube_to_pixel(cube: CubeCoord, size: f64) -> (f64, f64) {
    let x = size * ((3.0_f64).sqrt() * cube.x as f64 + (3.0_f64).sqrt() / 2.0 * cube.z as f64);
    let y = size * (1.5 * cube.z as f64);
    (x, y)
}

fn hexagon_corners(center: (f64, f64), size: f64) -> Vec<(f64, f64)> {
    let node_order = [
        NodeRef::North,
        NodeRef::NorthEast,
        NodeRef::SouthEast,
        NodeRef::South,
        NodeRef::SouthWest,
        NodeRef::NorthWest,
    ];
    node_order
        .iter()
        .map(|nr| node_position(center, size, *nr))
        .collect()
}

fn node_position(center: (f64, f64), size: f64, node_ref: NodeRef) -> (f64, f64) {
    let angle = match node_ref {
        NodeRef::North => -std::f64::consts::FRAC_PI_2,
        NodeRef::NorthEast => -std::f64::consts::FRAC_PI_6,
        NodeRef::SouthEast => std::f64::consts::FRAC_PI_6,
        NodeRef::South => std::f64::consts::FRAC_PI_2,
        NodeRef::SouthWest => 5.0 * std::f64::consts::FRAC_PI_6,
        NodeRef::NorthWest => -5.0 * std::f64::consts::FRAC_PI_6,
    };
    let (cx, cy) = center;
    (cx + size * angle.cos(), cy + size * angle.sin())
}

fn bounds(points: &[(f64, f64)]) -> Result<(f64, f64, f64, f64), &'static str> {
    if points.is_empty() {
        return Err("no points");
    }
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (x, y) in points {
        min_x = min_x.min(*x);
        max_x = max_x.max(*x);
        min_y = min_y.min(*y);
        max_y = max_y.max(*y);
    }
    Ok((min_x, max_x, min_y, max_y))
}

fn average(points: &[(f64, f64)]) -> (f64, f64) {
    let (sum_x, sum_y) = points
        .iter()
        .fold((0.0, 0.0), |acc, (x, y)| (acc.0 + x, acc.1 + y));
    let n = points.len() as f64;
    (sum_x / n, sum_y / n)
}

#[derive(Clone, Copy)]
enum TileKind {
    Land,
    Water,
    Port,
}

struct PortInfo {
    id: u16,
    resource: Option<Resource>,
    nodes: Vec<u16>,
}

fn dock_node_refs(direction: Direction) -> (NodeRef, NodeRef) {
    match direction {
        Direction::West => (NodeRef::NorthWest, NodeRef::SouthWest),
        Direction::NorthWest => (NodeRef::North, NodeRef::NorthWest),
        Direction::NorthEast => (NodeRef::NorthEast, NodeRef::North),
        Direction::East => (NodeRef::SouthEast, NodeRef::NorthEast),
        Direction::SouthEast => (NodeRef::South, NodeRef::SouthEast),
        Direction::SouthWest => (NodeRef::SouthWest, NodeRef::South),
    }
}

fn port_trade_label(resource: Option<Resource>) -> String {
    resource
        .map(|res| format!("{:?} 2:1", res))
        .unwrap_or_else(|| "Any 3:1".to_string())
}
