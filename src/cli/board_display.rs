use std::collections::{HashMap, HashSet, VecDeque};

use crate::board::{EdgeId, NodeId};
use crate::coords::{CubeCoord, cube_to_offset};
use crate::game::game::Game;
use crate::game::players::PlayerState;
use crate::types::{Color, Resource};

pub fn display_board(game: &Game) {
    display_hex_grid(game);
}

#[derive(Debug, Clone)]
pub struct RenderedBoard {
    pub text: String,
    pub node_spans: HashMap<NodeId, NodeSpan>,
}

#[derive(Debug, Clone, Copy)]
pub struct NodeSpan {
    pub row: usize,
    pub col_start: usize,
    pub len: usize,
}

// Grid position structure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GridPos {
    row: usize,
    col: usize,
}

// Render board as a string (for TUI)
pub fn render_board_to_string(game: &Game) -> String {
    render_board(game).text
}

pub fn render_board(game: &Game) -> RenderedBoard {
    let robber_coord = game
        .state
        .map
        .land_tiles
        .iter()
        .find(|(_, tile)| tile.id == game.state.robber_tile)
        .map(|(coord, _)| *coord);

    // Build coordinate to display position mapping
    let coord_to_pos = build_coordinate_position_map(&game.state.map.land_tiles);

    // Get tile content strings for each position (0-18)
    // Pad all values to exactly 5 characters for consistent alignment
    let mut resource_strings = vec!["     ".to_string(); 19];
    let mut tile_id_strings = vec!["     ".to_string(); 19];

    for (coord, &pos) in &coord_to_pos {
        if let Some(land_tile) = game.state.map.land_tiles.get(coord) {
            let r = land_tile
                .resource
                .map(|r| resource_to_char(r))
                .unwrap_or('D');
            let n = land_tile
                .number
                .map(|n| n.to_string())
                .unwrap_or_else(|| String::new());
            let is_robber = robber_coord == Some(*coord);

            // Build combined display containing number + resource (and robber if present)
            let mut display = String::new();
            if !n.is_empty() {
                display.push_str(&n);
            }
            display.push(r);
            if is_robber {
                display.push_str("🔴");
            }

            resource_strings[pos] = center_pad(&display, 5);
            tile_id_strings[pos] = center_pad(&format!("{:02}", land_tile.id), 5);
        }
    }

    // Build roads by edge (normalized)
    let roads_by_edge: HashMap<EdgeId, usize> = game
        .state
        .road_occupancy
        .iter()
        .map(|(edge, player_idx)| {
            let normalized = if edge.0 < edge.1 {
                *edge
            } else {
                (edge.1, edge.0)
            };
            (normalized, *player_idx)
        })
        .collect();

    // Prepare node labels so placeholders can be replaced by padded node ids
    let node_labels = build_default_node_labels();

    // Template with placeholders
    let template = r#"                  
                                       p  {p06}  p
                                        {v47}_____{v45}
                                       /         \
                          p           /           \          
                    {p05}   {v44}______{v43}    {r00}    {v46}______{v48} p
                           /         \    {n00}    /         \  {p07} 
                       p  /           \           /           \  p   
                {v42}______{v40}    {r01}    {v21}_______{v19}    {r02}    {v49}______{v50}
               /         \    {n01}    /         \    {n02}    /         \
              /           \           /           \           /           \
            {v41}    {r03}    {v18}_______{v16}    {r04}    {v20}_______{v22}    {r05}    {v51}
             \    {n03}    /         \    {n04}    /         \    {n05}    /
              \           /           \           /           \           /
             p {v39}_______{v17}    {r06}    {v05}_________{v00}    {r07}   {v23}_______{v52} p
       {p04}   /         \    {n06}    /         \    {n07}    /         \    {p08}
              /           \           /           \           /           \
          p {v38}    {r08}    {v15}________{v04}    {r09}    {v01}_________{v06}    {r10}    {v53} p
             \    {n08}    /         \    {n09}    /         \    {n10}    /
              \           /           \           /           \           /
               {v37}_______{v14}    {r11}    {v03}_________{v02}    {r12}    {v07}________{v24}
               /         \    {n11}    /         \    {n12}    /         \
              /           \           /           \           /           \
          p {v36}    {r13}    {v13}_______{v12}    {r14}    {v09}_________{v08}    {r15}    {v25} p
             \    {n13}    /         \    {n14}    /         \    {n15}    /
        {p03} \           /           \           /           \           /  {p00}
               {v35}_______{v34}    {r16}    {v11}_______{v10}    {r17}    {v27}_______{v26} p
              p          \    {n16}    /         \    {n17}    /  
                          \           /           \           /
                           {v33}_______{v32}    {r18}    {v29}_______{v28}
                          p        p \    {n18}    / p      p
                             {p02}    \           /    {p01}
                                       {v31}_______{v30}
                              
"#;

    // Replace resource and number placeholders with fixed-width strings
    let mut output = template.to_string();
    for i in 0..19 {
        let r_placeholder = format!("{{r{:02}}}", i);
        let n_placeholder = format!("{{n{:02}}}", i);
        output = output.replace(&r_placeholder, &resource_strings[i]);
        output = output.replace(&n_placeholder, &tile_id_strings[i]);
    }

    // Replace port placeholders with resource labels
    for (port_id, port) in &game.state.map.ports_by_id {
        let placeholder = format!("{{p{:02}}}", port_id);
        if output.contains(&placeholder) {
            let (ratio, resource_initial) = match port.resource {
                Some(res) => ("2", res.to_string().chars().next().unwrap_or(' ')),
                None => ("3", 'A'),
            };
            let label = format!("{}:1 {}", ratio, resource_initial);
            let padded = center_pad(&label, 5);
            output = output.replace(&placeholder, &padded);
        }
    }

    // Convert to character grid so we can replace node placeholders with variable-length labels
    let mut grid: Vec<Vec<char>> = output.lines().map(|line| line.chars().collect()).collect();
    let mut node_positions: HashMap<NodeId, GridPos> = HashMap::new();
    let mut node_spans: HashMap<NodeId, NodeSpan> = HashMap::new();

    for (row_idx, line) in grid.iter_mut().enumerate() {
        replace_node_placeholders(
            row_idx,
            line,
            &node_labels,
            &mut node_positions,
            &mut node_spans,
        );
    }

    let mut node_label_positions: HashSet<(usize, usize)> = HashSet::new();
    for span in node_spans.values() {
        for offset in 0..span.len {
            let col = span.col_start + offset;
            node_label_positions.insert((span.row, col));
        }
    }

    color_roads_on_grid(
        &mut grid,
        &roads_by_edge,
        &node_positions,
        &node_spans,
        &node_label_positions,
        &game.state.players,
    );

    // Convert grid back to string
    let text = grid
        .iter()
        .map(|line| line.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");

    RenderedBoard { text, node_spans }
}

// Display board with visual markers for settlements, cities, roads, and ports ON THE GRID
pub fn display_hex_grid(game: &Game) {
    println!("\n{}", "=".repeat(80));
    println!("BOARD GRID:");
    println!("{}", "=".repeat(80));

    let output = render_board(game).text;
    println!("{}", output);

    // Legend
    println!("\nLEGEND:");
    println!("  Resources: W=Wood, B=Brick, S=Sheep, H=Wheat, O=Ore, D=Desert");
    println!("  🔴 = Robber location");
    println!("  Structures: node ids color-highlighted by owning player");
    println!("  Cities: same as settlements but bold when rendered in the TUI");
    println!("  Roads: Marked on edges with player color");
    println!("  Ports: Identified by surrounding harbor labels");
}

// Build mapping from tile position (0-18) to grid center coordinates
fn build_coordinate_position_map(
    land_tiles: &HashMap<CubeCoord, crate::board::LandTile>,
) -> HashMap<CubeCoord, usize> {
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
        labels.insert(node_id, format_node_label(node_id));
    }
    labels
}

fn format_node_label(node_id: NodeId) -> String {
    node_id.to_string()
}

fn center_pad(content: &str, width: usize) -> String {
    let char_count = content.chars().count();
    if char_count >= width {
        return content.to_string();
    }
    let total_padding = width - char_count;
    let left = total_padding / 2;
    let right = total_padding - left;
    format!("{}{}{}", " ".repeat(left), content, " ".repeat(right))
}

fn replace_node_placeholders(
    row_idx: usize,
    line: &mut Vec<char>,
    labels: &HashMap<NodeId, String>,
    node_positions: &mut HashMap<NodeId, GridPos>,
    node_spans: &mut HashMap<NodeId, NodeSpan>,
) {
    const PLACEHOLDER_LEN: usize = 5;
    let mut col: usize = 0;

    while col + PLACEHOLDER_LEN <= line.len() {
        if line[col] == '{' && line[col + 1] == 'v' && line[col + 4] == '}' {
            let tens = line[col + 2];
            let ones = line[col + 3];
            if tens.is_ascii_digit() && ones.is_ascii_digit() {
                let node_id = (((tens as u8 - b'0') * 10) + (ones as u8 - b'0')) as NodeId;
                let label = labels
                    .get(&node_id)
                    .cloned()
                    .unwrap_or_else(|| node_id.to_string());
                let replacement: Vec<char> = label.chars().collect();
                let start_col = col;
                let inserted_len = replacement.len();
                let center_col = start_col + inserted_len / 2;
                node_positions.insert(
                    node_id,
                    GridPos {
                        row: row_idx,
                        col: center_col,
                    },
                );
                node_spans.insert(
                    node_id,
                    NodeSpan {
                        row: row_idx,
                        col_start: start_col,
                        len: inserted_len,
                    },
                );
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
    node_spans: &HashMap<NodeId, NodeSpan>,
    node_label_positions: &HashSet<(usize, usize)>,
    players: &[PlayerState],
) {
    for (edge, player_idx) in roads_by_edge {
        if let Some(player) = players.get(*player_idx) {
            let road_char = color_to_char_lowercase(player.color);
            let (start_node, end_node) = *edge;
            if let (Some(start_center), Some(start_span), Some(end_span)) = (
                node_positions.get(&start_node),
                node_spans.get(&start_node),
                node_spans.get(&end_node),
            ) {
                if let Some(path) = find_edge_path(grid, *start_center, *start_span, *end_span) {
                    for pos in path {
                        if !node_label_positions.contains(&(pos.row, pos.col)) {
                            grid[pos.row][pos.col] = road_char;
                        }
                    }
                }
            }
        }
    }
}

fn find_edge_path(
    grid: &[Vec<char>],
    start: GridPos,
    start_span: NodeSpan,
    end_span: NodeSpan,
) -> Option<Vec<GridPos>> {
    let mut queue = VecDeque::new();
    let mut visited: HashSet<(usize, usize)> = HashSet::new();
    let mut parent: HashMap<(usize, usize), (usize, usize)> = HashMap::new();

    for seed_pos in span_adjacent_positions(start_span, grid) {
        for neighbor in edge_neighbors(seed_pos, grid) {
            let key = (neighbor.row, neighbor.col);
            if visited.insert(key) {
                parent.insert(key, (seed_pos.row, seed_pos.col));
                queue.push_back(neighbor);
            }
        }
    }

    while let Some(pos) = queue.pop_front() {
        if is_adjacent_to_span(pos, end_span) {
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
            if is_edge_char(ch) {
                neighbors.push(GridPos { row, col });
            }
        }
    }
    neighbors
}

fn is_edge_char(ch: char) -> bool {
    matches!(ch, '_' | '/' | '\\' | 'r' | 'b' | 'o' | 'w')
}

fn span_adjacent_positions(span: NodeSpan, grid: &[Vec<char>]) -> Vec<GridPos> {
    let mut positions = Vec::new();
    let row = span.row;
    if row >= grid.len() {
        return positions;
    }
    let line = &grid[row];
    let start = span.col_start;
    let end = span.col_start + span.len.saturating_sub(1);
    for col in start..=end {
        if col < line.len() {
            positions.push(GridPos { row, col });
        }
    }
    positions
}

fn is_adjacent_to_span(pos: GridPos, span: NodeSpan) -> bool {
    let dr = pos.row as i32 - span.row as i32;
    if dr.abs() > 1 {
        return false;
    }
    let span_start = span.col_start as i32;
    let span_end = (span.col_start + span.len.saturating_sub(1)) as i32;
    let col = pos.col as i32;
    col >= span_start - 1 && col <= span_end + 1
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
