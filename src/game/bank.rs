use rand::seq::SliceRandom;

use crate::game::resources::{COST_DEVELOPMENT, ResourceBundle, ResourceError};
use crate::types::{DevelopmentCard, Resource};

#[derive(Debug, Clone)]
pub struct Bank {
    resources: ResourceBundle,
    development_deck: Vec<DevelopmentCard>,
}

impl Bank {
    pub fn standard(rng: &mut impl rand::Rng) -> Self {
        let mut deck = build_development_deck();
        deck.shuffle(rng);
        Self {
            resources: ResourceBundle::from_counts([19, 19, 19, 19, 19]),
            development_deck: deck,
        }
    }

    pub fn resources(&self) -> &ResourceBundle {
        &self.resources
    }

    pub fn receive(&mut self, bundle: &ResourceBundle) {
        let mut updated = self.resources;
        updated.add_bundle(bundle);
        self.resources = updated;
    }

    pub fn dispense(&mut self, bundle: &ResourceBundle) -> Result<(), ResourceError> {
        let mut updated = self.resources;
        updated.subtract_bundle(bundle)?;
        self.resources = updated;
        Ok(())
    }

    pub fn draw_development_card(&mut self) -> Option<DevelopmentCard> {
        self.development_deck.pop()
    }

    pub fn buy_development_card(
        &mut self,
        rng: &mut impl rand::Rng,
        player_resources: &mut ResourceBundle,
    ) -> Result<Option<DevelopmentCard>, ResourceError> {
        player_resources.subtract_bundle(&COST_DEVELOPMENT)?;
        self.resources.add_bundle(&COST_DEVELOPMENT);
        if self.development_deck.is_empty() {
            return Ok(None);
        }
        // Deck is already shuffled, but to keep things interesting reshuffle leftovers occasionally.
        self.development_deck.shuffle(rng);
        Ok(self.development_deck.pop())
    }

    pub fn available(&self, resource: Resource) -> u8 {
        self.resources
            .iter()
            .find(|(r, _)| *r == resource)
            .map(|(_, v)| v)
            .unwrap_or(0)
    }

    pub fn development_deck_len(&self) -> usize {
        self.development_deck.len()
    }
}

fn build_development_deck() -> Vec<DevelopmentCard> {
    use DevelopmentCard::*;
    const DISTRIBUTION: &[(DevelopmentCard, usize)] = &[
        (Knight, 14),
        (VictoryPoint, 5),
        (RoadBuilding, 2),
        (YearOfPlenty, 2),
        (Monopoly, 2),
    ];

    let mut deck = Vec::with_capacity(25);
    for (card, count) in DISTRIBUTION {
        for _ in 0..*count {
            deck.push(*card);
        }
    }
    deck
}
