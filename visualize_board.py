#!/usr/bin/env python3
"""
Generate images of the hexagonal structure of the Catan board for base and mini maps.
"""

import matplotlib.pyplot as plt
import matplotlib.patches as patches
import numpy as np
from typing import List, Tuple, Dict, Optional
from dataclasses import dataclass
from collections import defaultdict

@dataclass
class CubeCoord:
    x: int
    y: int
    z: int
    
    def __post_init__(self):
        assert self.x + self.y + self.z == 0, "Cube coordinates must sum to zero"
    
    def __hash__(self):
        return hash((self.x, self.y, self.z))
    
    def __eq__(self, other):
        return (self.x, self.y, self.z) == (other.x, other.y, other.z)

class TileTemplate:
    LAND = "Land"
    WATER = "Water"
    PORT = "Port"

# Node reference enum
class NodeRef:
    North = "North"
    NorthEast = "NorthEast"
    SouthEast = "SouthEast"
    South = "South"
    SouthWest = "SouthWest"
    NorthWest = "NorthWest"

# Base map topology
BASE_TOPOLOGY = [
    ((0, 0, 0), TileTemplate.LAND),
    ((1, -1, 0), TileTemplate.LAND),
    ((0, -1, 1), TileTemplate.LAND),
    ((-1, 0, 1), TileTemplate.LAND),
    ((-1, 1, 0), TileTemplate.LAND),
    ((0, 1, -1), TileTemplate.LAND),
    ((1, 0, -1), TileTemplate.LAND),
    ((2, -2, 0), TileTemplate.LAND),
    ((1, -2, 1), TileTemplate.LAND),
    ((0, -2, 2), TileTemplate.LAND),
    ((-1, -1, 2), TileTemplate.LAND),
    ((-2, 0, 2), TileTemplate.LAND),
    ((-2, 1, 1), TileTemplate.LAND),
    ((-2, 2, 0), TileTemplate.LAND),
    ((-1, 2, -1), TileTemplate.LAND),
    ((0, 2, -2), TileTemplate.LAND),
    ((1, 1, -2), TileTemplate.LAND),
    ((2, 0, -2), TileTemplate.LAND),
    ((2, -1, -1), TileTemplate.LAND),
    ((3, -3, 0), TileTemplate.PORT),
    ((2, -3, 1), TileTemplate.WATER),
    ((1, -3, 2), TileTemplate.PORT),
    ((0, -3, 3), TileTemplate.WATER),
    ((-1, -2, 3), TileTemplate.PORT),
    ((-2, -1, 3), TileTemplate.WATER),
    ((-3, 0, 3), TileTemplate.PORT),
    ((-3, 1, 2), TileTemplate.WATER),
    ((-3, 2, 1), TileTemplate.PORT),
    ((-3, 3, 0), TileTemplate.WATER),
    ((-2, 3, -1), TileTemplate.PORT),
    ((-1, 3, -2), TileTemplate.WATER),
    ((0, 3, -3), TileTemplate.PORT),
    ((1, 2, -3), TileTemplate.WATER),
    ((2, 1, -3), TileTemplate.PORT),
    ((3, 0, -3), TileTemplate.WATER),
    ((3, -1, -2), TileTemplate.PORT),
    ((3, -2, -1), TileTemplate.WATER),
]

# Mini map topology
MINI_TOPOLOGY = [
    ((0, 0, 0), TileTemplate.LAND),
    ((1, -1, 0), TileTemplate.LAND),
    ((0, -1, 1), TileTemplate.LAND),
    ((-1, 0, 1), TileTemplate.LAND),
    ((-1, 1, 0), TileTemplate.LAND),
    ((0, 1, -1), TileTemplate.LAND),
    ((1, 0, -1), TileTemplate.LAND),
    ((2, -2, 0), TileTemplate.WATER),
    ((1, -2, 1), TileTemplate.WATER),
    ((0, -2, 2), TileTemplate.WATER),
    ((-1, -1, 2), TileTemplate.WATER),
    ((-2, 0, 2), TileTemplate.WATER),
    ((-2, 1, 1), TileTemplate.WATER),
    ((-2, 2, 0), TileTemplate.WATER),
    ((-1, 2, -1), TileTemplate.WATER),
    ((0, 2, -2), TileTemplate.WATER),
    ((1, 1, -2), TileTemplate.WATER),
    ((2, 0, -2), TileTemplate.WATER),
    ((2, -1, -1), TileTemplate.WATER),
]

def cube_to_pixel(cube: CubeCoord, size: float = 1.0) -> Tuple[float, float]:
    """
    Convert cube coordinates to pixel coordinates for flat-top hexagons.
    """
    x = size * (np.sqrt(3) * cube.x + np.sqrt(3) / 2 * cube.z)
    y = size * (3.0 / 2.0 * cube.z)
    return (x, y)

def hexagon_corners(center_x: float, center_y: float, size: float) -> List[Tuple[float, float]]:
    """
    Generate the 6 corners of a flat-top hexagon in the same orientation
    used by catanatron: start at North (-pi/2) and move clockwise through
    NE, SE, S, SW, NW.
    """
    node_order = [
        NodeRef.North,
        NodeRef.NorthEast,
        NodeRef.SouthEast,
        NodeRef.South,
        NodeRef.SouthWest,
        NodeRef.NorthWest,
    ]
    node_angles = {
        NodeRef.North: -np.pi / 2,
        NodeRef.NorthEast: -np.pi / 6,
        NodeRef.SouthEast: np.pi / 6,
        NodeRef.South: np.pi / 2,
        NodeRef.SouthWest: 5 * np.pi / 6,
        NodeRef.NorthWest: -5 * np.pi / 6,
    }
    corners = []
    for node in node_order:
        angle = node_angles[node]
        x = center_x + size * np.cos(angle)
        y = center_y + size * np.sin(angle)
        corners.append((x, y))
    return corners

def get_node_position(center_x: float, center_y: float, size: float, node_ref: str) -> Tuple[float, float]:
    """
    Get the position of a node (corner) based on node reference.
    """
    corners = hexagon_corners(center_x, center_y, size)
    node_order = [
        NodeRef.North,
        NodeRef.NorthEast,
        NodeRef.SouthEast,
        NodeRef.South,
        NodeRef.SouthWest,
        NodeRef.NorthWest,
    ]
    node_map = {ref: idx for idx, ref in enumerate(node_order)}
    return corners[node_map[node_ref]]

def draw_hexagon(ax, center_x: float, center_y: float, size: float, 
                 color: str, edgecolor: str = 'black', linewidth: float = 1.5,
                 alpha: float = 1.0, label: str = None):
    """
    Draw a hexagon at the given center position.
    """
    corners = hexagon_corners(center_x, center_y, size)
    hex = patches.Polygon(corners, closed=True, facecolor=color, 
                         edgecolor=edgecolor, linewidth=linewidth, alpha=alpha)
    ax.add_patch(hex)
    
    # Add label if provided
    if label:
        ax.text(center_x, center_y, label, ha='center', va='center', 
               fontsize=8, fontweight='bold', color='black')

# Mini map node IDs - extracted from node_ids.rs
MINI_NODE_IDS = {
    ((-2, 0, 2), NodeRef.North): 14,
    ((-2, 0, 2), NodeRef.NorthEast): 13,
    ((-2, 0, 2), NodeRef.NorthWest): 37,
    ((-2, 0, 2), NodeRef.South): 35,
    ((-2, 0, 2), NodeRef.SouthEast): 34,
    ((-2, 0, 2), NodeRef.SouthWest): 36,
    ((-2, 1, 1), NodeRef.North): 17,
    ((-2, 1, 1), NodeRef.NorthEast): 15,
    ((-2, 1, 1), NodeRef.NorthWest): 39,
    ((-2, 1, 1), NodeRef.South): 37,
    ((-2, 1, 1), NodeRef.SouthEast): 14,
    ((-2, 1, 1), NodeRef.SouthWest): 38,
    ((-2, 2, 0), NodeRef.North): 40,
    ((-2, 2, 0), NodeRef.NorthEast): 18,
    ((-2, 2, 0), NodeRef.NorthWest): 42,
    ((-2, 2, 0), NodeRef.South): 39,
    ((-2, 2, 0), NodeRef.SouthEast): 17,
    ((-2, 2, 0), NodeRef.SouthWest): 41,
    ((-1, -1, 2), NodeRef.North): 12,
    ((-1, -1, 2), NodeRef.NorthEast): 11,
    ((-1, -1, 2), NodeRef.NorthWest): 13,
    ((-1, -1, 2), NodeRef.South): 33,
    ((-1, -1, 2), NodeRef.SouthEast): 32,
    ((-1, -1, 2), NodeRef.SouthWest): 34,
    ((-1, 0, 1), NodeRef.North): 4,
    ((-1, 0, 1), NodeRef.NorthEast): 3,
    ((-1, 0, 1), NodeRef.NorthWest): 15,
    ((-1, 0, 1), NodeRef.South): 13,
    ((-1, 0, 1), NodeRef.SouthEast): 12,
    ((-1, 0, 1), NodeRef.SouthWest): 14,
    ((-1, 1, 0), NodeRef.North): 16,
    ((-1, 1, 0), NodeRef.NorthEast): 5,
    ((-1, 1, 0), NodeRef.NorthWest): 18,
    ((-1, 1, 0), NodeRef.South): 15,
    ((-1, 1, 0), NodeRef.SouthEast): 4,
    ((-1, 1, 0), NodeRef.SouthWest): 17,
    ((-1, 2, -1), NodeRef.North): 43,
    ((-1, 2, -1), NodeRef.NorthEast): 21,
    ((-1, 2, -1), NodeRef.NorthWest): 44,
    ((-1, 2, -1), NodeRef.South): 18,
    ((-1, 2, -1), NodeRef.SouthEast): 16,
    ((-1, 2, -1), NodeRef.SouthWest): 40,
    ((0, -2, 2), NodeRef.North): 10,
    ((0, -2, 2), NodeRef.NorthEast): 29,
    ((0, -2, 2), NodeRef.NorthWest): 11,
    ((0, -2, 2), NodeRef.South): 31,
    ((0, -2, 2), NodeRef.SouthEast): 30,
    ((0, -2, 2), NodeRef.SouthWest): 32,
    ((0, -1, 1), NodeRef.North): 2,
    ((0, -1, 1), NodeRef.NorthEast): 9,
    ((0, -1, 1), NodeRef.NorthWest): 3,
    ((0, -1, 1), NodeRef.South): 11,
    ((0, -1, 1), NodeRef.SouthEast): 10,
    ((0, -1, 1), NodeRef.SouthWest): 12,
    ((0, 0, 0), NodeRef.North): 0,
    ((0, 0, 0), NodeRef.NorthEast): 1,
    ((0, 0, 0), NodeRef.NorthWest): 5,
    ((0, 0, 0), NodeRef.South): 3,
    ((0, 0, 0), NodeRef.SouthEast): 2,
    ((0, 0, 0), NodeRef.SouthWest): 4,
    ((0, 1, -1), NodeRef.North): 19,
    ((0, 1, -1), NodeRef.NorthEast): 20,
    ((0, 1, -1), NodeRef.NorthWest): 21,
    ((0, 1, -1), NodeRef.South): 5,
    ((0, 1, -1), NodeRef.SouthEast): 0,
    ((0, 1, -1), NodeRef.SouthWest): 16,
    ((0, 2, -2), NodeRef.North): 45,
    ((0, 2, -2), NodeRef.NorthEast): 46,
    ((0, 2, -2), NodeRef.NorthWest): 47,
    ((0, 2, -2), NodeRef.South): 21,
    ((0, 2, -2), NodeRef.SouthEast): 19,
    ((0, 2, -2), NodeRef.SouthWest): 43,
    ((1, -2, 1), NodeRef.North): 8,
    ((1, -2, 1), NodeRef.NorthEast): 27,
    ((1, -2, 1), NodeRef.NorthWest): 9,
    ((1, -2, 1), NodeRef.South): 29,
    ((1, -2, 1), NodeRef.SouthEast): 28,
    ((1, -2, 1), NodeRef.SouthWest): 10,
    ((1, -1, 0), NodeRef.North): 6,
    ((1, -1, 0), NodeRef.NorthEast): 7,
    ((1, -1, 0), NodeRef.NorthWest): 1,
    ((1, -1, 0), NodeRef.South): 9,
    ((1, -1, 0), NodeRef.SouthEast): 8,
    ((1, -1, 0), NodeRef.SouthWest): 2,
    ((1, 0, -1), NodeRef.North): 22,
    ((1, 0, -1), NodeRef.NorthEast): 23,
    ((1, 0, -1), NodeRef.NorthWest): 20,
    ((1, 0, -1), NodeRef.South): 1,
    ((1, 0, -1), NodeRef.SouthEast): 6,
    ((1, 0, -1), NodeRef.SouthWest): 0,
    ((1, 1, -2), NodeRef.North): 48,
    ((1, 1, -2), NodeRef.NorthEast): 49,
    ((1, 1, -2), NodeRef.NorthWest): 46,
    ((1, 1, -2), NodeRef.South): 20,
    ((1, 1, -2), NodeRef.SouthEast): 22,
    ((1, 1, -2), NodeRef.SouthWest): 19,
    ((2, -2, 0), NodeRef.North): 24,
    ((2, -2, 0), NodeRef.NorthEast): 25,
    ((2, -2, 0), NodeRef.NorthWest): 7,
    ((2, -2, 0), NodeRef.South): 27,
    ((2, -2, 0), NodeRef.SouthEast): 26,
    ((2, -2, 0), NodeRef.SouthWest): 8,
    ((2, -1, -1), NodeRef.North): 52,
    ((2, -1, -1), NodeRef.NorthEast): 53,
    ((2, -1, -1), NodeRef.NorthWest): 23,
    ((2, -1, -1), NodeRef.South): 7,
    ((2, -1, -1), NodeRef.SouthEast): 24,
    ((2, -1, -1), NodeRef.SouthWest): 6,
    ((2, 0, -2), NodeRef.North): 50,
    ((2, 0, -2), NodeRef.NorthEast): 51,
    ((2, 0, -2), NodeRef.NorthWest): 49,
    ((2, 0, -2), NodeRef.South): 23,
    ((2, 0, -2), NodeRef.SouthEast): 52,
    ((2, 0, -2), NodeRef.SouthWest): 22,
}

# Base map node IDs - extracted from node_ids.rs (abbreviated, full version in actual file)
BASE_NODE_IDS = {
    ((-3, 0, 3), NodeRef.North): 36, ((-3, 0, 3), NodeRef.NorthEast): 35, ((-3, 0, 3), NodeRef.NorthWest): 71,
    ((-3, 0, 3), NodeRef.South): 69, ((-3, 0, 3), NodeRef.SouthEast): 68, ((-3, 0, 3), NodeRef.SouthWest): 70,
    ((-3, 1, 2), NodeRef.North): 38, ((-3, 1, 2), NodeRef.NorthEast): 37, ((-3, 1, 2), NodeRef.NorthWest): 73,
    ((-3, 1, 2), NodeRef.South): 71, ((-3, 1, 2), NodeRef.SouthEast): 36, ((-3, 1, 2), NodeRef.SouthWest): 72,
    ((-3, 2, 1), NodeRef.North): 41, ((-3, 2, 1), NodeRef.NorthEast): 39, ((-3, 2, 1), NodeRef.NorthWest): 75,
    ((-3, 2, 1), NodeRef.South): 73, ((-3, 2, 1), NodeRef.SouthEast): 38, ((-3, 2, 1), NodeRef.SouthWest): 74,
    ((-3, 3, 0), NodeRef.North): 76, ((-3, 3, 0), NodeRef.NorthEast): 42, ((-3, 3, 0), NodeRef.NorthWest): 78,
    ((-3, 3, 0), NodeRef.South): 75, ((-3, 3, 0), NodeRef.SouthEast): 41, ((-3, 3, 0), NodeRef.SouthWest): 77,
    ((-2, -1, 3), NodeRef.North): 34, ((-2, -1, 3), NodeRef.NorthEast): 33, ((-2, -1, 3), NodeRef.NorthWest): 35,
    ((-2, -1, 3), NodeRef.South): 67, ((-2, -1, 3), NodeRef.SouthEast): 66, ((-2, -1, 3), NodeRef.SouthWest): 68,
    ((-2, 0, 2), NodeRef.North): 14, ((-2, 0, 2), NodeRef.NorthEast): 13, ((-2, 0, 2), NodeRef.NorthWest): 37,
    ((-2, 0, 2), NodeRef.South): 35, ((-2, 0, 2), NodeRef.SouthEast): 34, ((-2, 0, 2), NodeRef.SouthWest): 36,
    ((-2, 1, 1), NodeRef.North): 17, ((-2, 1, 1), NodeRef.NorthEast): 15, ((-2, 1, 1), NodeRef.NorthWest): 39,
    ((-2, 1, 1), NodeRef.South): 37, ((-2, 1, 1), NodeRef.SouthEast): 14, ((-2, 1, 1), NodeRef.SouthWest): 38,
    ((-2, 2, 0), NodeRef.North): 40, ((-2, 2, 0), NodeRef.NorthEast): 18, ((-2, 2, 0), NodeRef.NorthWest): 42,
    ((-2, 2, 0), NodeRef.South): 39, ((-2, 2, 0), NodeRef.SouthEast): 17, ((-2, 2, 0), NodeRef.SouthWest): 41,
    ((-2, 3, -1), NodeRef.North): 79, ((-2, 3, -1), NodeRef.NorthEast): 44, ((-2, 3, -1), NodeRef.NorthWest): 80,
    ((-2, 3, -1), NodeRef.South): 42, ((-2, 3, -1), NodeRef.SouthEast): 40, ((-2, 3, -1), NodeRef.SouthWest): 76,
    ((-1, -2, 3), NodeRef.North): 32, ((-1, -2, 3), NodeRef.NorthEast): 31, ((-1, -2, 3), NodeRef.NorthWest): 33,
    ((-1, -2, 3), NodeRef.South): 65, ((-1, -2, 3), NodeRef.SouthEast): 64, ((-1, -2, 3), NodeRef.SouthWest): 66,
    ((-1, -1, 2), NodeRef.North): 12, ((-1, -1, 2), NodeRef.NorthEast): 11, ((-1, -1, 2), NodeRef.NorthWest): 13,
    ((-1, -1, 2), NodeRef.South): 33, ((-1, -1, 2), NodeRef.SouthEast): 32, ((-1, -1, 2), NodeRef.SouthWest): 34,
    ((-1, 0, 1), NodeRef.North): 4, ((-1, 0, 1), NodeRef.NorthEast): 3, ((-1, 0, 1), NodeRef.NorthWest): 15,
    ((-1, 0, 1), NodeRef.South): 13, ((-1, 0, 1), NodeRef.SouthEast): 12, ((-1, 0, 1), NodeRef.SouthWest): 14,
    ((-1, 1, 0), NodeRef.North): 16, ((-1, 1, 0), NodeRef.NorthEast): 5, ((-1, 1, 0), NodeRef.NorthWest): 18,
    ((-1, 1, 0), NodeRef.South): 15, ((-1, 1, 0), NodeRef.SouthEast): 4, ((-1, 1, 0), NodeRef.SouthWest): 17,
    ((-1, 2, -1), NodeRef.North): 43, ((-1, 2, -1), NodeRef.NorthEast): 21, ((-1, 2, -1), NodeRef.NorthWest): 44,
    ((-1, 2, -1), NodeRef.South): 18, ((-1, 2, -1), NodeRef.SouthEast): 16, ((-1, 2, -1), NodeRef.SouthWest): 40,
    ((-1, 3, -2), NodeRef.North): 81, ((-1, 3, -2), NodeRef.NorthEast): 47, ((-1, 3, -2), NodeRef.NorthWest): 82,
    ((-1, 3, -2), NodeRef.South): 44, ((-1, 3, -2), NodeRef.SouthEast): 43, ((-1, 3, -2), NodeRef.SouthWest): 79,
    ((0, -3, 3), NodeRef.North): 30, ((0, -3, 3), NodeRef.NorthEast): 61, ((0, -3, 3), NodeRef.NorthWest): 31,
    ((0, -3, 3), NodeRef.South): 63, ((0, -3, 3), NodeRef.SouthEast): 62, ((0, -3, 3), NodeRef.SouthWest): 64,
    ((0, -2, 2), NodeRef.North): 10, ((0, -2, 2), NodeRef.NorthEast): 29, ((0, -2, 2), NodeRef.NorthWest): 11,
    ((0, -2, 2), NodeRef.South): 31, ((0, -2, 2), NodeRef.SouthEast): 30, ((0, -2, 2), NodeRef.SouthWest): 32,
    ((0, -1, 1), NodeRef.North): 2, ((0, -1, 1), NodeRef.NorthEast): 9, ((0, -1, 1), NodeRef.NorthWest): 3,
    ((0, -1, 1), NodeRef.South): 11, ((0, -1, 1), NodeRef.SouthEast): 10, ((0, -1, 1), NodeRef.SouthWest): 12,
    ((0, 0, 0), NodeRef.North): 0, ((0, 0, 0), NodeRef.NorthEast): 1, ((0, 0, 0), NodeRef.NorthWest): 5,
    ((0, 0, 0), NodeRef.South): 3, ((0, 0, 0), NodeRef.SouthEast): 2, ((0, 0, 0), NodeRef.SouthWest): 4,
    ((0, 1, -1), NodeRef.North): 19, ((0, 1, -1), NodeRef.NorthEast): 20, ((0, 1, -1), NodeRef.NorthWest): 21,
    ((0, 1, -1), NodeRef.South): 5, ((0, 1, -1), NodeRef.SouthEast): 0, ((0, 1, -1), NodeRef.SouthWest): 16,
    ((0, 2, -2), NodeRef.North): 45, ((0, 2, -2), NodeRef.NorthEast): 46, ((0, 2, -2), NodeRef.NorthWest): 47,
    ((0, 2, -2), NodeRef.South): 21, ((0, 2, -2), NodeRef.SouthEast): 19, ((0, 2, -2), NodeRef.SouthWest): 43,
    ((0, 3, -3), NodeRef.North): 83, ((0, 3, -3), NodeRef.NorthEast): 84, ((0, 3, -3), NodeRef.NorthWest): 85,
    ((0, 3, -3), NodeRef.South): 47, ((0, 3, -3), NodeRef.SouthEast): 45, ((0, 3, -3), NodeRef.SouthWest): 81,
    ((1, -3, 2), NodeRef.North): 28, ((1, -3, 2), NodeRef.NorthEast): 59, ((1, -3, 2), NodeRef.NorthWest): 29,
    ((1, -3, 2), NodeRef.South): 61, ((1, -3, 2), NodeRef.SouthEast): 60, ((1, -3, 2), NodeRef.SouthWest): 30,
    ((1, -2, 1), NodeRef.North): 8, ((1, -2, 1), NodeRef.NorthEast): 27, ((1, -2, 1), NodeRef.NorthWest): 9,
    ((1, -2, 1), NodeRef.South): 29, ((1, -2, 1), NodeRef.SouthEast): 28, ((1, -2, 1), NodeRef.SouthWest): 10,
    ((1, -1, 0), NodeRef.North): 6, ((1, -1, 0), NodeRef.NorthEast): 7, ((1, -1, 0), NodeRef.NorthWest): 1,
    ((1, -1, 0), NodeRef.South): 9, ((1, -1, 0), NodeRef.SouthEast): 8, ((1, -1, 0), NodeRef.SouthWest): 2,
    ((1, 0, -1), NodeRef.North): 22, ((1, 0, -1), NodeRef.NorthEast): 23, ((1, 0, -1), NodeRef.NorthWest): 20,
    ((1, 0, -1), NodeRef.South): 1, ((1, 0, -1), NodeRef.SouthEast): 6, ((1, 0, -1), NodeRef.SouthWest): 0,
    ((1, 1, -2), NodeRef.North): 48, ((1, 1, -2), NodeRef.NorthEast): 49, ((1, 1, -2), NodeRef.NorthWest): 46,
    ((1, 1, -2), NodeRef.South): 20, ((1, 1, -2), NodeRef.SouthEast): 22, ((1, 1, -2), NodeRef.SouthWest): 19,
    ((1, 2, -3), NodeRef.North): 86, ((1, 2, -3), NodeRef.NorthEast): 87, ((1, 2, -3), NodeRef.NorthWest): 84,
    ((1, 2, -3), NodeRef.South): 46, ((1, 2, -3), NodeRef.SouthEast): 48, ((1, 2, -3), NodeRef.SouthWest): 45,
    ((2, -3, 1), NodeRef.North): 26, ((2, -3, 1), NodeRef.NorthEast): 57, ((2, -3, 1), NodeRef.NorthWest): 27,
    ((2, -3, 1), NodeRef.South): 59, ((2, -3, 1), NodeRef.SouthEast): 58, ((2, -3, 1), NodeRef.SouthWest): 28,
    ((2, -2, 0), NodeRef.North): 24, ((2, -2, 0), NodeRef.NorthEast): 25, ((2, -2, 0), NodeRef.NorthWest): 7,
    ((2, -2, 0), NodeRef.South): 27, ((2, -2, 0), NodeRef.SouthEast): 26, ((2, -2, 0), NodeRef.SouthWest): 8,
    ((2, -1, -1), NodeRef.North): 52, ((2, -1, -1), NodeRef.NorthEast): 53, ((2, -1, -1), NodeRef.NorthWest): 23,
    ((2, -1, -1), NodeRef.South): 7, ((2, -1, -1), NodeRef.SouthEast): 24, ((2, -1, -1), NodeRef.SouthWest): 6,
    ((2, 0, -2), NodeRef.North): 50, ((2, 0, -2), NodeRef.NorthEast): 51, ((2, 0, -2), NodeRef.NorthWest): 49,
    ((2, 0, -2), NodeRef.South): 23, ((2, 0, -2), NodeRef.SouthEast): 52, ((2, 0, -2), NodeRef.SouthWest): 22,
    ((2, 1, -3), NodeRef.North): 88, ((2, 1, -3), NodeRef.NorthEast): 89, ((2, 1, -3), NodeRef.NorthWest): 87,
    ((2, 1, -3), NodeRef.South): 49, ((2, 1, -3), NodeRef.SouthEast): 50, ((2, 1, -3), NodeRef.SouthWest): 48,
    ((3, -3, 0), NodeRef.North): 54, ((3, -3, 0), NodeRef.NorthEast): 55, ((3, -3, 0), NodeRef.NorthWest): 25,
    ((3, -3, 0), NodeRef.South): 57, ((3, -3, 0), NodeRef.SouthEast): 56, ((3, -3, 0), NodeRef.SouthWest): 26,
    ((3, -2, -1), NodeRef.North): 94, ((3, -2, -1), NodeRef.NorthEast): 95, ((3, -2, -1), NodeRef.NorthWest): 53,
    ((3, -2, -1), NodeRef.South): 25, ((3, -2, -1), NodeRef.SouthEast): 54, ((3, -2, -1), NodeRef.SouthWest): 24,
    ((3, -1, -2), NodeRef.North): 92, ((3, -1, -2), NodeRef.NorthEast): 93, ((3, -1, -2), NodeRef.NorthWest): 51,
    ((3, -1, -2), NodeRef.South): 53, ((3, -1, -2), NodeRef.SouthEast): 94, ((3, -1, -2), NodeRef.SouthWest): 52,
    ((3, 0, -3), NodeRef.North): 90, ((3, 0, -3), NodeRef.NorthEast): 91, ((3, 0, -3), NodeRef.NorthWest): 89,
    ((3, 0, -3), NodeRef.South): 51, ((3, 0, -3), NodeRef.SouthEast): 92, ((3, 0, -3), NodeRef.SouthWest): 50,
}

def visualize_map(topology: List[Tuple[Tuple[int, int, int], str]], 
                  title: str, filename: str, hex_size: float = 0.5,
                  node_ids: Optional[Dict] = None):
    """
    Visualize a Catan map topology with node IDs labeled.
    """
    fig, ax = plt.subplots(1, 1, figsize=(20, 20))
    ax.set_aspect('equal')
    ax.axis('off')
    
    # Color scheme
    colors = {
        TileTemplate.LAND: '#8B4513',  # Brown for land
        TileTemplate.WATER: '#4169E1',  # Royal blue for water
        TileTemplate.PORT: '#FFD700',   # Gold for ports
    }
    
    # Track node positions to avoid duplicate labels
    # Group by node_id first, then average positions (since same node appears in multiple tiles)
    node_id_to_positions = defaultdict(list)  # node_id -> list of (x, y) positions
    
    # Convert topology to CubeCoord objects and draw
    coords_pixels = []
    for (x, y, z), tile_type in topology:
        cube = CubeCoord(x, y, z)
        px, py = cube_to_pixel(cube, hex_size)
        coords_pixels.append((px, py, tile_type))
        
        # Draw hexagon
        color = colors.get(tile_type, 'gray')
        draw_hexagon(ax, px, py, hex_size, color, edgecolor='black', 
                    linewidth=1.5, alpha=0.8)
        
        # Collect node data if node_ids provided
        if node_ids:
            coord_key = (x, y, z)
            for node_ref in [NodeRef.North, NodeRef.NorthEast, NodeRef.SouthEast, 
                            NodeRef.South, NodeRef.SouthWest, NodeRef.NorthWest]:
                key = (coord_key, node_ref)
                if key in node_ids:
                    node_id = node_ids[key]
                    node_x, node_y = get_node_position(px, py, hex_size, node_ref)
                    node_id_to_positions[node_id].append((node_x, node_y))
    
    # For each node_id, use the average position (or first if only one)
    # This handles cases where the same node appears in multiple tiles with slightly different positions
    node_positions = {}  # node_id -> (x, y) position (final mapping)
    for node_id, positions in node_id_to_positions.items():
        if len(positions) == 1:
            node_positions[node_id] = positions[0]
        else:
            # Average the positions to get the true center
            avg_x = sum(x for x, y in positions) / len(positions)
            avg_y = sum(y for x, y in positions) / len(positions)
            node_positions[node_id] = (avg_x, avg_y)
    
    # Draw all node labels
    for node_id, (node_x, node_y) in node_positions.items():
        # Draw a small circle for the node
        circle = patches.Circle((node_x, node_y), hex_size * 0.15, 
                               facecolor='white', edgecolor='black', linewidth=1.5, zorder=10)
        ax.add_patch(circle)
        # Add node ID label
        ax.text(node_x, node_y, str(node_id), ha='center', va='center', 
               fontsize=7, fontweight='bold', color='black', zorder=11)
    
    # Set axis limits with padding
    if coords_pixels:
        xs = [x for x, _, _ in coords_pixels]
        ys = [y for _, y, _ in coords_pixels]
        x_min, x_max = min(xs), max(xs)
        y_min, y_max = min(ys), max(ys)
        padding = hex_size * 2
        ax.set_xlim(x_min - padding, x_max + padding)
        ax.set_ylim(y_min - padding, y_max + padding)
    
    # Add title
    ax.set_title(title, fontsize=20, fontweight='bold', pad=20)
    
    # Add legend
    from matplotlib.patches import Patch
    legend_elements = [
        Patch(facecolor=colors[TileTemplate.LAND], label='Land', alpha=0.8),
        Patch(facecolor=colors[TileTemplate.WATER], label='Water', alpha=0.8),
        Patch(facecolor=colors[TileTemplate.PORT], label='Port', alpha=0.8),
    ]
    ax.legend(handles=legend_elements, loc='upper right', fontsize=12, framealpha=0.9)
    
    # Add statistics
    land_count = sum(1 for _, t in topology if t == TileTemplate.LAND)
    water_count = sum(1 for _, t in topology if t == TileTemplate.WATER)
    port_count = sum(1 for _, t in topology if t == TileTemplate.PORT)
    node_count = len(node_positions)
    stats_text = f'Total Tiles: {len(topology)}\nLand: {land_count} | Water: {water_count} | Ports: {port_count}\nNodes: {node_count}'
    ax.text(0.02, 0.98, stats_text, transform=ax.transAxes, 
           fontsize=10, verticalalignment='top', 
           bbox=dict(boxstyle='round', facecolor='wheat', alpha=0.8))
    
    plt.tight_layout()
    plt.savefig(filename, dpi=300, bbox_inches='tight')
    print(f"Saved {filename}")
    plt.close()

def main():
    print("Generating Catan board visualizations...")
    
    # Generate base map with node IDs
    visualize_map(BASE_TOPOLOGY, 
                  "Catan Base Map - Hexagonal Structure with Node IDs", 
                  "catan_base_map.png",
                  hex_size=0.5,
                  node_ids=BASE_NODE_IDS)
    
    # Generate mini map with node IDs
    visualize_map(MINI_TOPOLOGY, 
                  "Catan Mini Map - Hexagonal Structure with Node IDs", 
                  "catan_mini_map.png",
                  hex_size=0.6,
                  node_ids=MINI_NODE_IDS)
    
    print("Done!")

if __name__ == "__main__":
    main()

