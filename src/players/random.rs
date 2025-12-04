use crate::game::action::GameAction;
use crate::game::game::Game;
use crate::players::BasePlayer;
use rand::seq::SliceRandom;

#[derive(Clone)]
pub struct RandomPlayer;

impl BasePlayer for RandomPlayer {
    fn decide(&self, _game: &Game, actions: &[GameAction]) -> Option<GameAction> {
        let mut rng = rand::thread_rng();
        actions.choose(&mut rng).cloned()
    }
}
