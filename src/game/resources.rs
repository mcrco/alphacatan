use std::fmt;

use serde::{Deserialize, Serialize};

use crate::types::Resource;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceBundle {
    counts: [u8; Resource::ALL.len()],
}

impl Default for ResourceBundle {
    fn default() -> Self {
        Self::zero()
    }
}

impl ResourceBundle {
    pub const fn from_counts(counts: [u8; 5]) -> Self {
        Self { counts }
    }

    pub const fn zero() -> Self {
        Self {
            counts: [0; Resource::ALL.len()],
        }
    }

    pub fn total(&self) -> u32 {
        self.counts.iter().map(|&v| v as u32).sum()
    }

    pub fn add(&mut self, resource: Resource, amount: u8) {
        let idx = resource_index(resource);
        self.counts[idx] = self.counts[idx].saturating_add(amount);
    }

    pub fn add_bundle(&mut self, other: &ResourceBundle) {
        for (idx, value) in other.counts.iter().enumerate() {
            self.counts[idx] = self.counts[idx].saturating_add(*value);
        }
    }

    pub fn subtract(&mut self, resource: Resource, amount: u8) -> Result<(), ResourceError> {
        let idx = resource_index(resource);
        if self.counts[idx] < amount {
            return Err(ResourceError::InsufficientResource {
                resource,
                available: self.counts[idx],
                requested: amount,
            });
        }
        self.counts[idx] -= amount;
        Ok(())
    }

    pub fn subtract_bundle(&mut self, other: &ResourceBundle) -> Result<(), ResourceError> {
        if !self.can_afford(other) {
            return Err(ResourceError::InsufficientBundle);
        }
        for (idx, value) in other.counts.iter().enumerate() {
            self.counts[idx] -= *value;
        }
        Ok(())
    }

    pub fn can_afford(&self, other: &ResourceBundle) -> bool {
        self.counts
            .iter()
            .zip(other.counts.iter())
            .all(|(have, need)| have >= need)
    }

    pub fn is_empty(&self) -> bool {
        self.counts.iter().all(|&value| value == 0)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Resource, u8)> + '_ {
        Resource::ALL.into_iter().zip(self.counts.iter().copied())
    }

    pub fn counts(&self) -> [u8; Resource::ALL.len()] {
        self.counts
    }

    pub fn get(&self, resource: Resource) -> u8 {
        self.counts[resource_index(resource)]
    }
}

impl fmt::Display for ResourceBundle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = vec![];
        for (resource, amount) in self.iter() {
            if amount > 0 {
                parts.push(format!("{amount}x{resource}"));
            }
        }
        write!(f, "{}", parts.join(", "))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    #[error("insufficient {resource:?}: have {available}, need {requested}")]
    InsufficientResource {
        resource: Resource,
        available: u8,
        requested: u8,
    },
    #[error("insufficient resources to cover bundle")]
    InsufficientBundle,
}

const fn resource_index(resource: Resource) -> usize {
    match resource {
        Resource::Wood => 0,
        Resource::Brick => 1,
        Resource::Sheep => 2,
        Resource::Wheat => 3,
        Resource::Ore => 4,
    }
}

pub const COST_ROAD: ResourceBundle = ResourceBundle::from_counts([1, 1, 0, 0, 0]);
pub const COST_SETTLEMENT: ResourceBundle = ResourceBundle::from_counts([1, 1, 1, 1, 0]);
pub const COST_CITY: ResourceBundle = ResourceBundle::from_counts([0, 0, 0, 2, 3]);
pub const COST_DEVELOPMENT: ResourceBundle = ResourceBundle::from_counts([0, 0, 1, 1, 1]);
