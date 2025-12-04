use crate::game::{action::GameAction, game::Game};

pub trait BasePlayer {
    fn decide(&self, game: &Game, actions: &[GameAction]) -> Option<GameAction>;
}
