use serde::{Deserialize, Serialize};

use crate::board::{EdgeId, NodeId};
use crate::game::resources::ResourceBundle;
use crate::types::{ActionType, DevelopmentCard, Resource};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct GameAction {
    pub player_index: usize,
    pub action_type: ActionType,
    pub payload: ActionPayload,
}

impl GameAction {
    pub fn new(player_index: usize, action_type: ActionType) -> Self {
        Self {
            player_index,
            action_type,
            payload: ActionPayload::None,
        }
    }

    pub fn with_payload(mut self, payload: ActionPayload) -> Self {
        self.payload = payload;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ActionPayload {
    None,
    Node(NodeId),
    Edge(EdgeId),
    Dice(u8, u8),
    Resources(ResourceBundle),
    Resource(Resource),
    Trade {
        give: ResourceBundle,
        receive: ResourceBundle,
        partner: Option<usize>,
    },
    MaritimeTrade {
        give: ResourceBundle,
        receive: Resource,
    },
    DevelopmentCard(DevelopmentCard),
    Robber {
        tile_id: u16,
        victim: Option<usize>,
        resource: Option<Resource>,
    },
}

impl Default for ActionPayload {
    fn default() -> Self {
        ActionPayload::None
    }
}
