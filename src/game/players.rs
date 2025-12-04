use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::board::{EdgeId, NodeId};
use crate::game::resources::{ResourceBundle, ResourceError};
use crate::types::{Color, DevelopmentCard};

pub const MAX_ROADS: usize = 15;
pub const MAX_SETTLEMENTS: usize = 5;
pub const MAX_CITIES: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub color: Color,
    pub resources: ResourceBundle,
    pub dev_cards: Vec<DevelopmentCard>,
    pub fresh_dev_cards: Vec<DevelopmentCard>,
    pub roads: HashSet<EdgeId>,
    pub settlements: HashSet<NodeId>,
    pub cities: HashSet<NodeId>,
    pub victory_points: u8,
    pub knights_played: u8,
    pub has_longest_road: bool,
    pub has_largest_army: bool,
    pub has_rolled: bool,
    pub has_played_dev_card_this_turn: bool,
    pub played_dev_cards: HashMap<DevelopmentCard, u32>,
}

impl PlayerState {
    pub fn new(color: Color) -> Self {
        Self {
            color,
            resources: ResourceBundle::zero(),
            dev_cards: Vec::new(),
            fresh_dev_cards: Vec::new(),
            roads: HashSet::new(),
            settlements: HashSet::new(),
            cities: HashSet::new(),
            victory_points: 0,
            knights_played: 0,
            has_longest_road: false,
            has_largest_army: false,
            has_rolled: false,
            has_played_dev_card_this_turn: false,
            played_dev_cards: HashMap::new(),
        }
    }

    pub fn reset_for_new_turn(&mut self) {
        self.dev_cards.extend(self.fresh_dev_cards.drain(..));
        self.has_rolled = false;
        self.has_played_dev_card_this_turn = false;
    }

    pub fn add_resources(&mut self, bundle: &ResourceBundle) {
        self.resources.add_bundle(bundle);
    }

    pub fn remove_resources(&mut self, bundle: &ResourceBundle) -> Result<(), ResourceError> {
        self.resources.subtract_bundle(bundle)
    }

    pub fn add_dev_card(&mut self, card: DevelopmentCard) {
        self.fresh_dev_cards.push(card);
        if matches!(card, DevelopmentCard::VictoryPoint) {
            self.victory_points += 1;
        }
    }

    pub fn record_dev_card_play(&mut self, card: DevelopmentCard) {
        *self.played_dev_cards.entry(card).or_insert(0) += 1;
        if matches!(card, DevelopmentCard::Knight) {
            self.knights_played += 1;
        }
        self.has_played_dev_card_this_turn = true;
    }

    pub fn matured_dev_card_count(&self, card: DevelopmentCard) -> usize {
        self.dev_cards.iter().filter(|c| **c == card).count()
    }

    pub fn can_play_dev_card(&self, card: DevelopmentCard) -> bool {
        if self.has_played_dev_card_this_turn {
            return false;
        }
        self.matured_dev_card_count(card) > 0
    }

    pub fn consume_dev_card(&mut self, card: DevelopmentCard) -> bool {
        if let Some(pos) = self.dev_cards.iter().position(|c| *c == card) {
            self.dev_cards.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn settlement_limit_reached(&self) -> bool {
        self.settlements.len() >= MAX_SETTLEMENTS
    }

    pub fn city_limit_reached(&self) -> bool {
        self.cities.len() >= MAX_CITIES
    }

    pub fn road_limit_reached(&self) -> bool {
        self.roads.len() >= MAX_ROADS
    }

    pub fn total_structures(&self) -> usize {
        self.settlements.len() + self.cities.len() + self.roads.len()
    }

    pub fn total_points(&self) -> u8 {
        let settlement_points = self.settlements.len() as u8;
        let city_points = (self.cities.len() as u8) * 2;
        settlement_points + city_points + self.victory_points + self.bonus_points()
    }

    pub fn public_points(&self) -> u8 {
        let settlement_points = self.settlements.len() as u8;
        let city_points = (self.cities.len() as u8) * 2;
        settlement_points + city_points + self.bonus_points()
    }

    pub fn bonus_points(&self) -> u8 {
        let mut bonus = 0;
        if self.has_longest_road {
            bonus += 2;
        }
        if self.has_largest_army {
            bonus += 2;
        }
        bonus
    }
}
