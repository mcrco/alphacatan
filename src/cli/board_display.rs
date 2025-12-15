use std::collections::{HashMap, HashSet, VecDeque};

use crate::board::{EdgeId, NodeId};
use crate::coords::{cube_to_offset, CubeCoord};
use crate::game::game::Game;
use crate::game::players::PlayerState;
use crate::game::state::Structure;
use crate::types::{Color, Resource};

pub fn display_board(game: &Game) {
    display_hex_grid(game);
}

// Grid position structure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GridPos {
    row: usize,
    col: usize,
}

// Render board as a string (for TUI)
pub fn render_board_to_string(game: &Game) -> String {
    let robber_coord = game.state.map.land_tiles.iter()
        .find(|(_, tile)| tile.id == game.state.robber_tile)
        .map(|(coord, _)| *coord);
    
    // Build coordinate to display position mapping
    let coord_to_pos = build_coordinate_position_map(&game.state.map.land_tiles);
    
    // Get tile content strings for each position (0-18)
    // Pad all values to exactly 5 characters for consistent alignment
    let mut resource_strings = vec!["     ".to_string(); 19];
    let mut number_strings = vec!["     ".to_string(); 19];
    
    for (coord, &pos) in &coord_to_pos {
        if let Some(land_tile) = game.state.map.land_tiles.get(coord) {
            let r = land_tile.resource.map(|r| resource_to_char(r)).unwrap_or('D');
            let n = land_tile.number.map(|n| n.to_string()).unwrap_or_else(|| String::new());
            let is_robber = robber_coord == Some(*coord);
            
            // Pad resource string to exactly 5 characters: "  X  "
            resource_strings[pos] = format!("  {}  ", r);
            
            // Pad number string to exactly 5 characters
            if is_robber {
                // For robber: pad to 5 characters accounting for emoji
                let robber_str = format!("{}🔴", n);
                let char_count = robber_str.chars().count();
                let padding = " ".repeat(5usize.saturating_sub(char_count));
                number_strings[pos] = format!("{}{}", robber_str, padding);
            } else {
                // For normal numbers: pad to 5 characters
                let char_count = n.chars().count();
                let padding = " ".repeat(5usize.saturating_sub(char_count));
                number_strings[pos] = format!("{}{}", n, padding);
            }
        }
    }
    
    // Build structures by node
    let structures_by_node: HashMap<NodeId, (usize, &Structure)> = game.state.node_occupancy.iter()
        .map(|(node_id, structure)| {
            let player_idx = match structure {
                Structure::Settlement { player } => *player,
                Structure::City { player } => *player,
            };
            (*node_id, (player_idx, structure))
        })
        .collect();
    
    // Build roads by edge (normalized)
    let roads_by_edge: HashMap<EdgeId, usize> = game.state.road_occupancy.iter()
        .map(|(edge, player_idx)| {
            let normalized = if edge.0 < edge.1 { *edge } else { (edge.1, edge.0) };
            (normalized, *player_idx)
        })
        .collect();
    
    // Build port nodes
    let mut port_nodes: HashSet<NodeId> = HashSet::new();
    for nodes in game.state.map.port_nodes.values() {
        for node_id in nodes {
            port_nodes.insert(*node_id);
        }
    }
    // Prepare node labels so placeholders can be replaced by either node ids or structure markers
    let mut node_labels = build_default_node_labels();
    for (node_id, (player_idx, structure)) in &structures_by_node {
        if let Some(player) = game.state.players.get(*player_idx) {
            let is_port_node = port_nodes.contains(node_id);
            let marker_char = match structure {
                Structure::Settlement { .. } => settlement_marker_char(player.color, is_port_node),
                Structure::City { .. } => color_to_char(player.color),
            };
            node_labels.insert(*node_id, format_structure_label(marker_char));
        }
    }
    
    // Template with placeholders
    let template = r#"                  
                               {v47}_____{v45}
                              /         \
                             /           \
                   {v44}______{v43}    {r00}    {v46}______{v48}
                  /         \    {n00}    /         \
                 /           \           /           \
       {v42}______{v40}    {r01}    {v21}_______{v19}    {r02}    {v49}______{v50}
      /         \    {n01}    /         \    {n02}    /         \
     /           \           /           \           /           \
   {v41}    {r03}    {v18}_______{v16}    {r04}    {v20}_______{v22}    {r05}    {v51}
    \    {n03}    /         \    {n04}    /         \    {n05}    /
     \           /           \           /           \           /
      {v39}_______{v17}    {r06}    {v05}_________{v00}   {r07}    {v23}_______{v52}
      /         \    {n06}    /         \    {n07}    /         \
     /           \           /           \           /           \
   {v38}    {r08}    {v15}________{v04}    {r09}    {v01}_________{v06}    {r10}    {v53}
    \    {n08}    /         \    {n09}    /         \    {n10}    /
     \           /           \           /           \           /
      {v37}_______{v14}    {r11}    {v03}_________{v02}    {r12}    {v07}________{v24}
      /         \    {n11}    /         \    {n12}    /         \
     /           \           /           \           /           \
   {v36}    {r13}    {v13}_______{v12}    {r14}    {v09}_________{v08}    {r15}    {v25}
    \    {n13}    /         \    {n14}    /         \    {n15}    /
     \           /           \           /           \           /
      {v35}_______{v34}    {r16}    {v11}_______{v10}    {r17}    {v27}_______{v26}
                \    {n16}    /         \    {n17}    /
                 \           /           \           /
                  {v33}_______{v32}    {r18}    {v29}_______{v28}
                            \    {n18}    /
                             \           /
                              {v31}_______{v30}
"#;
    
    // Replace resource and number placeholders with fixed-width strings
    let mut output = template.to_string();
    for i in 0..19 {
        let r_placeholder = format!("{{r{:02}}}", i);
        let n_placeholder = format!("{{n{:02}}}", i);
        output = output.replace(&r_placeholder, &resource_strings[i]);
        output = output.replace(&n_placeholder, &number_strings[i]);
    }
    
    // Convert to character grid so we can replace node placeholders with variable-length labels
    let mut grid: Vec<Vec<char>> = output.lines().map(|line| line.chars().collect()).collect();
    let mut node_positions: HashMap<NodeId, GridPos> = HashMap::new();
    
    for (row_idx, line) in grid.iter_mut().enumerate() {
        replace_node_placeholders(row_idx, line, &node_labels, &mut node_positions);
    }
    
    color_roads_on_grid(
        &mut grid,
        &roads_by_edge,
        &node_positions,
        &game.state.players,
    );
    
    // Convert grid back to string
    grid.iter().map(|line| line.iter().collect::<String>()).collect::<Vec<_>>().join("\n")
}

// Display board with visual markers for settlements, cities, roads, and ports ON THE GRID
pub fn display_hex_grid(game: &Game) {
    println!("\n{}", "=".repeat(80));
    println!("BOARD GRID:");
    println!("{}", "=".repeat(80));
    
    let output = render_board_to_string(game);
    println!("{}", output);
    
    // Legend
    println!("\nLEGEND:");
    println!("  Resources: W=Wood, B=Brick, S=Sheep, H=Wheat, O=Ore, D=Desert");
    println!("  🔴 = Robber location");
    println!("  Structures: r=Red settlement, R=Red city, b=Blue, o=Orange, w=White");
    println!("  Roads: Marked on edges with player color");
    println!("  Ports: Special markers (p/q/s/t) for port settlements");
}

// Build mapping from tile position (0-18) to grid center coordinates
fn build_coordinate_position_map(land_tiles: &HashMap<CubeCoord, crate::board::LandTile>) -> HashMap<CubeCoord, usize> {
    let mut coords: Vec<CubeCoord> = land_tiles.keys().copied().collect();
    
    coords.sort_by_key(|coord| {
        let (col, row) = cube_to_offset(*coord);
        (row, col)
    });
    
    let mut result = HashMap::new();
    for (pos, coord) in coords.iter().enumerate() {
        result.insert(*coord, pos);
    }
    
    result
}

fn resource_to_char(r: Resource) -> char {
    match r {
        Resource::Wood => 'W',
        Resource::Brick => 'B',
        Resource::Sheep => 'S',
        Resource::Wheat => 'H',
        Resource::Ore => 'O',
    }
}

fn color_to_char(c: Color) -> char {
    match c {
        Color::Red => 'R',
        Color::Blue => 'B',
        Color::Orange => 'O',
        Color::White => 'W',
    }
}

fn color_to_char_lowercase(c: Color) -> char {
    match c {
        Color::Red => 'r',
        Color::Blue => 'b',
        Color::Orange => 'o',
        Color::White => 'w',
    }
}

const MAX_TEMPLATE_NODE_ID: NodeId = 53;

fn build_default_node_labels() -> HashMap<NodeId, String> {
    let mut labels = HashMap::new();
    for node_id in 0..=MAX_TEMPLATE_NODE_ID {
        labels.insert(node_id, node_id.to_string());
    }
    labels
}

fn format_structure_label(marker: char) -> String {
    marker.to_string()
}

fn settlement_marker_char(color: Color, is_port: bool) -> char {
    let base = color_to_char_lowercase(color);
    if is_port {
        match base {
            'r' => 'p',
            'b' => 'q',
            'o' => 's',
            'w' => 't',
            _ => base,
        }
    } else {
        base
    }
}

fn replace_node_placeholders(
    row_idx: usize,
    line: &mut Vec<char>,
    labels: &HashMap<NodeId, String>,
    node_positions: &mut HashMap<NodeId, GridPos>,
) {
    const PLACEHOLDER_LEN: usize = 5;
    let mut col: usize = 0;
    
    while col + PLACEHOLDER_LEN <= line.len() {
        if line[col] == '{' && line[col + 1] == 'v' && line[col + 4] == '}' {
            let tens = line[col + 2];
            let ones = line[col + 3];
            if tens.is_ascii_digit() && ones.is_ascii_digit() {
                let node_id = (((tens as u8 - b'0') * 10) + (ones as u8 - b'0')) as NodeId;
                let label = labels.get(&node_id).cloned().unwrap_or_else(|| node_id.to_string());
                let replacement: Vec<char> = label.chars().collect();
                let center_col = col + replacement.len().saturating_sub(1) / 2;
                node_positions.insert(node_id, GridPos { row: row_idx, col: center_col });
                let inserted_len = replacement.len();
                line.splice(col..col + PLACEHOLDER_LEN, replacement);
                col += inserted_len;
                continue;
            }
        }
        col += 1;
    }
}

fn color_roads_on_grid(
    grid: &mut [Vec<char>],
    roads_by_edge: &HashMap<EdgeId, usize>,
    node_positions: &HashMap<NodeId, GridPos>,
    players: &[PlayerState],
) {
    for (edge, player_idx) in roads_by_edge {
        if let Some(player) = players.get(*player_idx) {
            let road_char = color_to_char_lowercase(player.color);
            let (start_node, end_node) = *edge;
            if let (Some(start), Some(end)) = (node_positions.get(&start_node), node_positions.get(&end_node)) {
                if let Some(path) = find_edge_path(grid, *start, *end) {
                    for pos in path {
                        grid[pos.row][pos.col] = road_char;
                    }
                }
            }
        }
    }
}

fn find_edge_path(grid: &[Vec<char>], start: GridPos, end: GridPos) -> Option<Vec<GridPos>> {
    let mut queue = VecDeque::new();
    let mut visited: HashSet<(usize, usize)> = HashSet::new();
    let mut parent: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
    
    for neighbor in edge_neighbors(start, grid) {
        let key = (neighbor.row, neighbor.col);
        visited.insert(key);
        parent.insert(key, (start.row, start.col));
        queue.push_back(neighbor);
    }
    
    while let Some(pos) = queue.pop_front() {
        if is_adjacent_to_node(pos, end) {
            return Some(reconstruct_path(pos, (start.row, start.col), &parent));
        }
        
        for neighbor in edge_neighbors(pos, grid) {
            let key = (neighbor.row, neighbor.col);
            if visited.insert(key) {
                parent.insert(key, (pos.row, pos.col));
                queue.push_back(neighbor);
            }
        }
    }
    
    None
}

fn edge_neighbors(origin: GridPos, grid: &[Vec<char>]) -> Vec<GridPos> {
    let mut neighbors = Vec::new();
    for dr in -1i32..=1 {
        for dc in -1i32..=1 {
            if dr == 0 && dc == 0 {
                continue;
            }
            let nr = origin.row as i32 + dr;
            let nc = origin.col as i32 + dc;
            if nr < 0 {
                continue;
            }
            let row = nr as usize;
            if row >= grid.len() {
                continue;
            }
            if nc < 0 {
                continue;
            }
            let col = nc as usize;
            if col >= grid[row].len() {
                continue;
            }
            let ch = grid[row][col];
            if ch == '_' || ch == '/' || ch == '\\' {
                neighbors.push(GridPos { row, col });
            }
        }
    }
    neighbors
}

fn is_adjacent_to_node(pos: GridPos, node: GridPos) -> bool {
    let dr = pos.row as i32 - node.row as i32;
    let dc = pos.col as i32 - node.col as i32;
    dr.abs() <= 1 && dc.abs() <= 1
}

fn reconstruct_path(
    mut current: GridPos,
    start: (usize, usize),
    parent: &HashMap<(usize, usize), (usize, usize)>,
) -> Vec<GridPos> {
    let mut path = Vec::new();
    path.push(current);
    while let Some(&(pr, pc)) = parent.get(&(current.row, current.col)) {
        if pr == start.0 && pc == start.1 {
            break;
        }
        current = GridPos { row: pr, col: pc };
        path.push(current);
    }
    path.reverse();
    path
}
