use std::collections::{HashMap, HashSet, VecDeque};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumString, Display, EnumIter,
)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum Direction {
    East,
    SouthEast,
    SouthWest,
    West,
    NorthWest,
    NorthEast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CubeCoord {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl CubeCoord {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        debug_assert!(x + y + z == 0, "cube coordinates must sum to zero");
        Self { x, y, z }
    }

    pub fn add(self, other: CubeCoord) -> Self {
        CubeCoord::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    pub fn neighbors(self) -> impl Iterator<Item = CubeCoord> {
        UNIT_VECTORS.iter().map(move |(_, vec)| self.add(*vec))
    }

    pub fn from_offset(x: i32, y: i32) -> Self {
        offset_to_cube((x, y))
    }
}

impl Default for CubeCoord {
    fn default() -> Self {
        CubeCoord::new(0, 0, 0)
    }
}

pub static UNIT_VECTORS: Lazy<HashMap<Direction, CubeCoord>> = Lazy::new(|| {
    use Direction::*;
    HashMap::from([
        (NorthEast, CubeCoord::new(1, 0, -1)),
        (SouthWest, CubeCoord::new(-1, 0, 1)),
        (NorthWest, CubeCoord::new(0, 1, -1)),
        (SouthEast, CubeCoord::new(0, -1, 1)),
        (East, CubeCoord::new(1, -1, 0)),
        (West, CubeCoord::new(-1, 1, 0)),
    ])
});

pub fn add(a: CubeCoord, b: CubeCoord) -> CubeCoord {
    a.add(b)
}

fn num_tiles_for(layer: i32) -> i32 {
    if layer == 0 {
        return 1;
    }
    6 * layer + num_tiles_for(layer - 1)
}

pub fn generate_coordinate_system(num_layers: i32) -> HashSet<CubeCoord> {
    let target = num_tiles_for(num_layers);
    let mut visited = HashSet::new();
    let mut queue = VecDeque::from([CubeCoord::new(0, 0, 0)]);

    while (visited.len() as i32) < target {
        let node = queue.pop_front().expect("queue should not be empty");
        if !visited.insert(node) {
            continue;
        }
        for neighbor in node.neighbors() {
            if !visited.contains(&neighbor) && !queue.contains(&neighbor) {
                queue.push_back(neighbor);
            }
        }
    }
    visited
}

pub fn cube_to_axial(cube: CubeCoord) -> (i32, i32) {
    (cube.x, cube.z)
}

pub fn cube_to_offset(cube: CubeCoord) -> (i32, i32) {
    let col = cube.x + (cube.z - (cube.z & 1)) / 2;
    (col, cube.z)
}

pub fn offset_to_cube(offset: (i32, i32)) -> CubeCoord {
    let x = offset.0 - (offset.1 - (offset.1 & 1)) / 2;
    let z = offset.1;
    let y = -x - z;
    CubeCoord::new(x, y, z)
}
