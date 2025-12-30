#![warn(clippy::all)]
#![deny(rust_2018_idioms)]

pub mod board;
pub mod cli;
pub mod coords;
pub mod env;
pub mod features;
pub mod game;
pub mod players;
pub mod types;

pub use board::CatanMap;
pub use board::MapType;
pub use board::Tile;
pub use env::{Observation, PlayerObservation, RustEnv, StepResult};
pub use game::{Game, GameConfig, GameState};
pub use types::Color;
