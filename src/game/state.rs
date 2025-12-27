use std::collections::{HashMap, HashSet, VecDeque};

use clap::error::Error;
use rand::{Rng, SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};

use crate::{
    board::{CatanMap, EdgeId, MapType, NodeId},
    types::{ActionPrompt, ActionType, Color, DevelopmentCard, Resource},
};

use super::{
    action::{ActionPayload, GameAction},
    bank::Bank,
    players::PlayerState,
    resources::{COST_CITY, COST_DEVELOPMENT, COST_ROAD, COST_SETTLEMENT, ResourceBundle},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub num_players: usize,
    pub map_type: MapType,
    pub vps_to_win: u8,
    pub seed: u64,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            num_players: 4,
            map_type: MapType::Base,
            vps_to_win: 10,
            seed: 42,
        }
    }
}

#[derive(Debug, Clone)]
pub enum GamePhase {
    Setup(SetupState),
    Playing,
    Completed { winner: Option<usize> },
}

#[derive(Debug, Clone)]
pub struct GameState {
    pub config: GameConfig,
    pub map: CatanMap,
    pub players: Vec<PlayerState>,
    pub bank: Bank,
    pub phase: GamePhase,
    pub pending_prompt: ActionPrompt,
    pub current_player: usize,
    turn_owner: usize,
    pub turn: u32,
    pub robber_tile: u16,
    pub last_roll: Option<(u8, u8)>,
    pub node_occupancy: HashMap<NodeId, Structure>,
    pub road_occupancy: HashMap<EdgeId, usize>,
    pub actions: Vec<GameAction>,
    available_actions: Vec<GameAction>,
    awaiting_roll: bool,
    discard_queue: VecDeque<usize>,
    discard_targets: HashMap<usize, u8>,
    road_building_player: Option<usize>,
    road_building_free_roads: u8,
    trade_state: Option<TradeState>,
    trade_queue: VecDeque<usize>,
    setup_pending_roads: HashMap<usize, NodeId>,
    rng: StdRng,
}

#[derive(Debug, Clone, Copy)]
pub enum Structure {
    Settlement { player: usize },
    City { player: usize },
}

#[derive(Debug, Clone)]
pub struct StepOutcome {
    pub events: Vec<GameEvent>,
    pub rewards: Vec<f32>,
    pub done: bool,
}

impl StepOutcome {
    fn empty(num_players: usize) -> Self {
        Self {
            events: Vec::new(),
            rewards: vec![0.0; num_players],
            done: false,
        }
    }
}

#[derive(Debug, Clone)]
struct TradeState {
    offerer: usize,
    give: ResourceBundle,
    receive: ResourceBundle,
    acceptees: HashSet<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameEvent {
    DiceRolled {
        player: usize,
        dice: (u8, u8),
        sum: u8,
    },
    ResourcesDistributed {
        player: usize,
        bundle: ResourceBundle,
    },
    BuiltRoad {
        player: usize,
        edge: EdgeId,
    },
    BuiltSettlement {
        player: usize,
        node: NodeId,
    },
    BuiltCity {
        player: usize,
        node: NodeId,
    },
    TurnAdvanced {
        next_player: usize,
    },
    GameWon {
        winner: usize,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum GameError {
    #[error("game already completed")]
    GameFinished,
    #[error("invalid player index {0}")]
    InvalidPlayer(usize),
    #[error("action by player {actual} but expected {expected}")]
    ActionOutOfTurn { expected: usize, actual: usize },
    #[error("action {action:?} invalid for prompt {prompt:?}")]
    InvalidPrompt {
        prompt: ActionPrompt,
        action: ActionType,
    },
    #[error("missing or invalid payload: {0}")]
    InvalidPayload(&'static str),
    #[error("node {0} already occupied")]
    NodeOccupied(NodeId),
    #[error("cannot build adjacent to another settlement")]
    DistanceRuleViolation,
    #[error("settlement must connect to existing network")]
    MustConnectToNetwork,
    #[error("edge not found on map")]
    EdgeNotFound,
    #[error("edge already occupied")]
    EdgeOccupied,
    #[error("insufficient resources")]
    InsufficientResources,
    #[error("bank resources unavailable")]
    BankOutOfResources,
    #[error("action not allowed at this stage")]
    IllegalAction,
}

impl GameState {
    pub fn new(config: GameConfig) -> Self {
        assert!(
            (2..=4).contains(&config.num_players),
            "Catan supports between 2 and 4 players"
        );

        let mut rng = StdRng::seed_from_u64(config.seed);
        let map = CatanMap::build_with_rng(config.map_type, &mut rng);
        let robber_tile = map
            .tiles_by_id
            .values()
            .find(|tile| tile.resource.is_none())
            .map(|tile| tile.id)
            .unwrap_or(0);
        let players = Color::ORDERED
            .iter()
            .take(config.num_players)
            .map(|color| PlayerState::new(*color))
            .collect::<Vec<_>>();

        let bank = Bank::standard(&mut rng);
        let setup_state = SetupState::new(config.num_players);
        let pending_prompt = setup_state
            .current_prompt()
            .unwrap_or(ActionPrompt::PlayTurn);
        let current_player = setup_state.current_player().unwrap_or(0);

        let mut state = Self {
            config,
            map,
            players,
            bank,
            phase: GamePhase::Setup(setup_state),
            pending_prompt,
            current_player,
            turn_owner: current_player,
            turn: 0,
            robber_tile,
            last_roll: None,
            node_occupancy: HashMap::new(),
            road_occupancy: HashMap::new(),
            actions: Vec::new(),
            available_actions: Vec::new(),
            awaiting_roll: false,
            discard_queue: VecDeque::new(),
            discard_targets: HashMap::new(),
            road_building_player: None,
            road_building_free_roads: 0,
            trade_state: None,
            trade_queue: VecDeque::new(),
            setup_pending_roads: HashMap::new(),
            rng,
        };
        state.refresh_available_actions();
        state
    }

    pub fn reset(&mut self) {
        *self = GameState::new(self.config.clone());
    }

    pub fn step(&mut self, mut action: GameAction) -> Result<StepOutcome, GameError> {
        if matches!(self.phase, GamePhase::Completed { .. }) {
            return Err(GameError::GameFinished);
        }
        if action.player_index >= self.players.len() {
            return Err(GameError::InvalidPlayer(action.player_index));
        }
        let mut outcome = StepOutcome::empty(self.players.len());
        if matches!(&self.phase, GamePhase::Setup(_)) {
            self.handle_setup_action(&mut action, &mut outcome)?
        } else {
            self.handle_play_action(&mut action, &mut outcome)?
        }
        self.actions.push(action);
        self.refresh_available_actions();
        if let GamePhase::Completed { winner } = self.phase {
            outcome.done = true;
            if let Some(winner_idx) = winner {
                outcome
                    .events
                    .push(GameEvent::GameWon { winner: winner_idx });
                for (idx, reward) in outcome.rewards.iter_mut().enumerate() {
                    if idx == winner_idx {
                        *reward = 1.0;
                    } else {
                        *reward = -1.0;
                    }
                }
            }
        }
        Ok(outcome)
    }

    pub fn legal_action_prompt(&self) -> ActionPrompt {
        self.pending_prompt
    }

    fn handle_setup_action(
        &mut self,
        action: &mut GameAction,
        outcome: &mut StepOutcome,
    ) -> Result<(), GameError> {
        let (current_player, prompt, is_second_settlement) = match &self.phase {
            GamePhase::Setup(state) => (
                state.current_player().ok_or(GameError::IllegalAction)?,
                state.current_prompt().unwrap_or(ActionPrompt::PlayTurn),
                state.is_second_settlement(),
            ),
            _ => return Err(GameError::IllegalAction),
        };

        if current_player != action.player_index {
            return Err(GameError::ActionOutOfTurn {
                expected: current_player,
                actual: action.player_index,
            });
        }
        self.pending_prompt = prompt;
        match (prompt, action.action_type) {
            (ActionPrompt::BuildInitialSettlement, ActionType::BuildSettlement) => {
                let node_id = match action.payload {
                    ActionPayload::Node(node) => node,
                    _ => return Err(GameError::InvalidPayload("expected node id")),
                };
                self.validate_settlement_location(action.player_index, node_id, false)?;
                self.place_settlement(action.player_index, node_id)?;
                if is_second_settlement {
                    self.award_starting_resources(action.player_index, node_id, outcome)?;
                }
                self.setup_pending_roads
                    .insert(action.player_index, node_id);
                outcome.events.push(GameEvent::BuiltSettlement {
                    player: action.player_index,
                    node: node_id,
                });
            }
            (ActionPrompt::BuildInitialRoad, ActionType::BuildRoad) => {
                let edge = match action.payload {
                    ActionPayload::Edge(edge) => edge,
                    _ => return Err(GameError::InvalidPayload("expected edge id")),
                };
                if let Some(anchor) = self.setup_pending_roads.get(&action.player_index) {
                    if !edge_contains_node(edge, *anchor) {
                        return Err(GameError::IllegalAction);
                    }
                }
                self.validate_road_location(action.player_index, edge, false)?;
                self.place_road(action.player_index, edge);
                self.setup_pending_roads.remove(&action.player_index);
                outcome.events.push(GameEvent::BuiltRoad {
                    player: action.player_index,
                    edge,
                });
            }
            _ => {
                return Err(GameError::InvalidPrompt {
                    prompt,
                    action: action.action_type,
                });
            }
        }

        let (next_player, next_prompt, setup_complete) = {
            let state = match &mut self.phase {
                GamePhase::Setup(state) => state,
                _ => unreachable!(),
            };
            state.advance();
            if state.is_complete() {
                (0, ActionPrompt::PlayTurn, true)
            } else {
                (
                    state.current_player().unwrap_or(0),
                    state.current_prompt().unwrap_or(ActionPrompt::PlayTurn),
                    false,
                )
            }
        };

        if setup_complete {
            self.phase = GamePhase::Playing;
            self.current_player = 0;
            self.turn_owner = 0;
            self.pending_prompt = ActionPrompt::PlayTurn;
            self.awaiting_roll = true;
        } else {
            self.current_player = next_player;
            self.pending_prompt = next_prompt;
        }
        Ok(())
    }

    fn handle_play_action(
        &mut self,
        action: &mut GameAction,
        outcome: &mut StepOutcome,
    ) -> Result<(), GameError> {
        if action.player_index != self.current_player {
            return Err(GameError::ActionOutOfTurn {
                expected: self.current_player,
                actual: action.player_index,
            });
        }

        match self.pending_prompt {
            ActionPrompt::PlayTurn => self.handle_turn_action(action, outcome)?,
            ActionPrompt::Discard => self.handle_discard_action(action)?,
            ActionPrompt::MoveRobber => self.handle_move_robber_action(action)?,
            ActionPrompt::DecideTrade => self.handle_trade_response_action(action)?,
            ActionPrompt::DecideAcceptees => self.handle_trade_confirmation_action(action)?,
            _ => {
                return Err(GameError::InvalidPrompt {
                    prompt: self.pending_prompt,
                    action: action.action_type,
                });
            }
        }

        self.check_victory();
        Ok(())
    }

    fn handle_turn_action(
        &mut self,
        action: &mut GameAction,
        outcome: &mut StepOutcome,
    ) -> Result<(), GameError> {
        match action.action_type {
            ActionType::Roll => {
                if !self.awaiting_roll {
                    return Err(GameError::IllegalAction);
                }
                let (d1, d2) = match action.payload {
                    ActionPayload::Dice(a, b) => (a.max(1).min(6), b.max(1).min(6)),
                    _ => (self.roll_die(), self.roll_die()),
                };
                let sum = d1 + d2;
                self.last_roll = Some((d1, d2));
                self.awaiting_roll = false;
                if let Some(player) = self.players.get_mut(action.player_index) {
                    player.has_rolled = true;
                }
                action.payload = ActionPayload::Dice(d1, d2);
                outcome.events.push(GameEvent::DiceRolled {
                    player: action.player_index,
                    dice: (d1, d2),
                    sum,
                });
                if sum != 7 {
                    self.distribute_resources(sum, outcome)?;
                    self.pending_prompt = ActionPrompt::PlayTurn;
                } else {
                    self.begin_discard_phase();
                }
            }
            ActionType::BuildRoad => {
                let use_free = self.road_building_player == Some(action.player_index)
                    && self.road_building_free_roads > 0;
                if !use_free {
                    self.ensure_can_act_after_roll()?;
                }
                let edge = match action.payload {
                    ActionPayload::Edge(edge) => edge,
                    _ => return Err(GameError::InvalidPayload("expected edge id")),
                };
                self.validate_road_location(action.player_index, edge, true)?;
                if !use_free {
                    self.pay_cost(action.player_index, &COST_ROAD)?;
                } else {
                    self.road_building_free_roads -= 1;
                    if self.road_building_free_roads == 0 {
                        self.road_building_player = None;
                    }
                }
                self.place_road(action.player_index, edge);
                outcome.events.push(GameEvent::BuiltRoad {
                    player: action.player_index,
                    edge,
                });
            }
            ActionType::BuildSettlement => {
                self.ensure_can_act_after_roll()?;
                let node_id = match action.payload {
                    ActionPayload::Node(node) => node,
                    _ => return Err(GameError::InvalidPayload("expected node id")),
                };
                self.validate_settlement_location(action.player_index, node_id, true)?;
                self.pay_cost(action.player_index, &COST_SETTLEMENT)?;
                self.place_settlement(action.player_index, node_id)?;
                outcome.events.push(GameEvent::BuiltSettlement {
                    player: action.player_index,
                    node: node_id,
                });
            }
            ActionType::BuildCity => {
                self.ensure_can_act_after_roll()?;
                let node_id = match action.payload {
                    ActionPayload::Node(node) => node,
                    _ => return Err(GameError::InvalidPayload("expected node id")),
                };
                self.upgrade_settlement_to_city(action.player_index, node_id)?;
                outcome.events.push(GameEvent::BuiltCity {
                    player: action.player_index,
                    node: node_id,
                });
            }
            ActionType::EndTurn => {
                self.ensure_can_act_after_roll()?;
                self.clear_trade_state();
                self.clear_road_building();
                self.advance_turn(outcome);
            }
            ActionType::BuyDevelopmentCard => {
                self.ensure_can_act_after_roll()?;
                self.buy_development_card(action.player_index)?;
            }
            ActionType::MaritimeTrade => {
                self.ensure_can_act_after_roll()?;
                let (give, receive) = match action.payload.clone() {
                    ActionPayload::MaritimeTrade { give, receive } => (give, receive),
                    _ => return Err(GameError::InvalidPayload("expected maritime trade payload")),
                };
                self.maritime_trade(action.player_index, give, receive)?;
            }
            ActionType::OfferTrade => {
                self.ensure_can_act_after_roll()?;
                let (give, receive) = match action.payload.clone() {
                    ActionPayload::Trade { give, receive, .. } => (give, receive),
                    _ => return Err(GameError::InvalidPayload("expected domestic trade payload")),
                };
                self.begin_trade(action.player_index, give, receive)?;
            }
            ActionType::PlayKnightCard => {
                self.play_knight_card(action.player_index)?;
            }
            ActionType::PlayYearOfPlenty => {
                let bundle = match action.payload.clone() {
                    ActionPayload::Resources(bundle) => bundle,
                    _ => {
                        return Err(GameError::InvalidPayload(
                            "expected resource bundle for year of plenty",
                        ));
                    }
                };
                self.play_year_of_plenty(action.player_index, bundle)?;
            }
            ActionType::PlayMonopoly => {
                let resource = match action.payload {
                    ActionPayload::Resource(resource) => resource,
                    _ => {
                        return Err(GameError::InvalidPayload(
                            "expected resource payload for monopoly",
                        ));
                    }
                };
                self.play_monopoly(action.player_index, resource)?;
            }
            ActionType::PlayRoadBuilding => {
                self.play_road_building(action.player_index)?;
            }
            _ => return Err(GameError::IllegalAction),
        }

        Ok(())
    }

    fn handle_discard_action(&mut self, action: &mut GameAction) -> Result<(), GameError> {
        if action.action_type != ActionType::Discard {
            return Err(GameError::InvalidPrompt {
                prompt: ActionPrompt::Discard,
                action: action.action_type,
            });
        }
        let Some(&required) = self.discard_targets.get(&action.player_index) else {
            return Err(GameError::IllegalAction);
        };
        let discarded_resource = if let ActionPayload::Resource(res) = action.payload {
            res
        } else {
            return Err(GameError::InvalidPayload(
                "invalid payload for discard action. expected resource",
            ));
        };
        let mut bundle = ResourceBundle::zero();
        bundle.add(discarded_resource, 1);
        self.players[action.player_index]
            .remove_resources(&bundle)
            .map_err(|_| GameError::InsufficientResources)?;
        self.bank.receive(&bundle);
        action.payload = ActionPayload::Resources(bundle);

        if required == 1 {
            self.discard_targets.remove(&action.player_index);
            if let Some(next) = self.discard_queue.pop_front() {
                self.current_player = next;
            } else {
                self.pending_prompt = ActionPrompt::MoveRobber;
                self.current_player = self.turn_owner;
            }
        } else {
            self.discard_targets
                .insert(action.player_index, required - 1);
        }
        Ok(())
    }

    fn handle_move_robber_action(&mut self, action: &mut GameAction) -> Result<(), GameError> {
        if action.action_type != ActionType::MoveRobber {
            return Err(GameError::InvalidPrompt {
                prompt: ActionPrompt::MoveRobber,
                action: action.action_type,
            });
        }
        let (tile_id, victim_idx) = match &action.payload {
            ActionPayload::Robber {
                tile_id, victim, ..
            } => (*tile_id, *victim),
            _ => return Err(GameError::InvalidPayload("expected robber payload")),
        };
        if !self.map.tiles_by_id.contains_key(&tile_id) {
            return Err(GameError::IllegalAction);
        }
        self.robber_tile = tile_id;
        if let Some(victim) = victim_idx {
            if victim >= self.players.len() {
                return Err(GameError::InvalidPlayer(victim));
            }
            if let Some(resource) = self.steal_random_resource(victim) {
                self.players[self.current_player].resources.add(resource, 1);
                action.payload = ActionPayload::Robber {
                    tile_id,
                    victim: Some(victim),
                    resource: Some(resource),
                };
            } else {
                action.payload = ActionPayload::Robber {
                    tile_id,
                    victim: Some(victim),
                    resource: None,
                };
            }
        }
        self.pending_prompt = ActionPrompt::PlayTurn;
        Ok(())
    }

    fn buy_development_card(&mut self, player_idx: usize) -> Result<(), GameError> {
        if self.bank.development_deck_len() == 0 {
            return Err(GameError::IllegalAction);
        }
        if !self.players[player_idx]
            .resources
            .can_afford(&COST_DEVELOPMENT)
        {
            return Err(GameError::InsufficientResources);
        }
        let card = self
            .bank
            .buy_development_card(&mut self.rng, &mut self.players[player_idx].resources)
            .map_err(|_| GameError::InsufficientResources)?;
        if let Some(card) = card {
            self.players[player_idx].add_dev_card(card);
        }
        Ok(())
    }

    fn ensure_dev_card_available(
        &mut self,
        player_idx: usize,
        card: DevelopmentCard,
    ) -> Result<(), GameError> {
        if !self.players[player_idx].can_play_dev_card(card) {
            return Err(GameError::IllegalAction);
        }
        if !self.players[player_idx].consume_dev_card(card) {
            return Err(GameError::IllegalAction);
        }
        self.players[player_idx].record_dev_card_play(card);
        Ok(())
    }

    fn play_knight_card(&mut self, player_idx: usize) -> Result<(), GameError> {
        self.ensure_dev_card_available(player_idx, DevelopmentCard::Knight)?;
        self.update_largest_army();
        self.pending_prompt = ActionPrompt::MoveRobber;
        self.current_player = player_idx;
        Ok(())
    }

    fn play_year_of_plenty(
        &mut self,
        player_idx: usize,
        bundle: ResourceBundle,
    ) -> Result<(), GameError> {
        let total = bundle.total();
        if total == 0 || total > 2 {
            return Err(GameError::InvalidPayload(
                "year of plenty must select one or two resources",
            ));
        }
        self.ensure_dev_card_available(player_idx, DevelopmentCard::YearOfPlenty)?;
        self.bank
            .dispense(&bundle)
            .map_err(|_| GameError::BankOutOfResources)?;
        self.players[player_idx].add_resources(&bundle);
        Ok(())
    }

    fn play_monopoly(&mut self, player_idx: usize, resource: Resource) -> Result<(), GameError> {
        self.ensure_dev_card_available(player_idx, DevelopmentCard::Monopoly)?;
        let mut stolen = ResourceBundle::zero();
        for (idx, player) in self.players.iter_mut().enumerate() {
            if idx == player_idx {
                continue;
            }
            let amount = player.resources.get(resource);
            if amount > 0 {
                player
                    .resources
                    .subtract(resource, amount)
                    .map_err(|_| GameError::InsufficientResources)?;
                stolen.add(resource, amount);
            }
        }
        if !stolen.is_empty() {
            self.players[player_idx].add_resources(&stolen);
        }
        Ok(())
    }

    fn play_road_building(&mut self, player_idx: usize) -> Result<(), GameError> {
        self.ensure_dev_card_available(player_idx, DevelopmentCard::RoadBuilding)?;
        self.road_building_player = Some(player_idx);
        self.road_building_free_roads = 2;
        Ok(())
    }

    fn clear_road_building(&mut self) {
        self.road_building_player = None;
        self.road_building_free_roads = 0;
    }

    fn maritime_trade(
        &mut self,
        player_idx: usize,
        give: ResourceBundle,
        receive: Resource,
    ) -> Result<(), GameError> {
        let (resource, amount) = self
            .single_resource_bundle(&give)
            .ok_or(GameError::IllegalAction)?;
        if resource == receive {
            return Err(GameError::IllegalAction);
        }
        let rate = self.maritime_rate(player_idx, resource);
        if amount != rate {
            return Err(GameError::IllegalAction);
        }
        if !self.players[player_idx].resources.can_afford(&give) {
            return Err(GameError::InsufficientResources);
        }
        self.players[player_idx]
            .remove_resources(&give)
            .map_err(|_| GameError::InsufficientResources)?;
        self.bank.receive(&give);
        let mut receive_bundle = ResourceBundle::zero();
        receive_bundle.add(receive, 1);
        self.bank
            .dispense(&receive_bundle)
            .map_err(|_| GameError::BankOutOfResources)?;
        self.players[player_idx].add_resources(&receive_bundle);
        Ok(())
    }

    fn single_resource_bundle(&self, bundle: &ResourceBundle) -> Option<(Resource, u8)> {
        let mut found: Option<(Resource, u8)> = None;
        for (resource, amount) in bundle.iter() {
            if amount == 0 {
                continue;
            }
            if let Some((existing, existing_amount)) = found {
                if resource != existing {
                    return None;
                }
                return Some((existing, existing_amount + amount));
            } else {
                found = Some((resource, amount));
            }
        }
        found
    }

    fn maritime_rate(&self, player_idx: usize, resource: Resource) -> u8 {
        if self.player_has_port(player_idx, Some(resource)) {
            return 2;
        }
        if self.player_has_port(player_idx, None) {
            return 3;
        }
        4
    }

    fn player_has_port(&self, player_idx: usize, port: Option<Resource>) -> bool {
        let Some(nodes) = self.map.port_nodes.get(&port) else {
            return false;
        };
        nodes
            .iter()
            .any(|node| self.node_owned_by(player_idx, *node))
    }

    fn node_owned_by(&self, player_idx: usize, node: NodeId) -> bool {
        match self.node_occupancy.get(&node) {
            Some(Structure::Settlement { player }) | Some(Structure::City { player }) => {
                *player == player_idx
            }
            _ => false,
        }
    }

    fn begin_trade(
        &mut self,
        player_idx: usize,
        give: ResourceBundle,
        receive: ResourceBundle,
    ) -> Result<(), GameError> {
        if give.is_empty() || receive.is_empty() {
            return Err(GameError::IllegalAction);
        }
        if self.trade_state.is_some() {
            return Err(GameError::IllegalAction);
        }
        if !self.players[player_idx].resources.can_afford(&give) {
            return Err(GameError::InsufficientResources);
        }
        let mut queue = VecDeque::new();
        for offset in 1..self.players.len() {
            queue.push_back((player_idx + offset) % self.players.len());
        }
        self.trade_state = Some(TradeState {
            offerer: player_idx,
            give,
            receive,
            acceptees: HashSet::new(),
        });
        self.trade_queue = queue;
        self.advance_trade_queue();
        Ok(())
    }

    fn advance_trade_queue(&mut self) {
        if let Some(next) = self.trade_queue.pop_front() {
            self.current_player = next;
            self.pending_prompt = ActionPrompt::DecideTrade;
            return;
        }
        if let Some(state) = &self.trade_state {
            if state.acceptees.is_empty() {
                let offerer = state.offerer;
                self.clear_trade_state();
                self.current_player = offerer;
                self.pending_prompt = ActionPrompt::PlayTurn;
            } else {
                let offerer = state.offerer;
                self.current_player = offerer;
                self.pending_prompt = ActionPrompt::DecideAcceptees;
            }
        } else {
            self.pending_prompt = ActionPrompt::PlayTurn;
            self.current_player = self.turn_owner;
        }
    }

    fn clear_trade_state(&mut self) {
        self.trade_state = None;
        self.trade_queue.clear();
    }

    fn handle_trade_response_action(&mut self, action: &mut GameAction) -> Result<(), GameError> {
        let Some(state) = self.trade_state.as_mut() else {
            return Err(GameError::IllegalAction);
        };
        if action.player_index == state.offerer {
            return Err(GameError::IllegalAction);
        }
        match action.action_type {
            ActionType::AcceptTrade => {
                if !self.players[action.player_index]
                    .resources
                    .can_afford(&state.receive)
                {
                    return Err(GameError::InsufficientResources);
                }
                state.acceptees.insert(action.player_index);
                self.advance_trade_queue();
                Ok(())
            }
            ActionType::RejectTrade => {
                self.advance_trade_queue();
                Ok(())
            }
            _ => Err(GameError::IllegalAction),
        }
    }

    fn handle_trade_confirmation_action(
        &mut self,
        action: &mut GameAction,
    ) -> Result<(), GameError> {
        let Some(state) = self.trade_state.clone() else {
            return Err(GameError::IllegalAction);
        };
        if action.player_index != state.offerer {
            return Err(GameError::IllegalAction);
        }
        match action.action_type {
            ActionType::CancelTrade => {
                self.clear_trade_state();
                self.pending_prompt = ActionPrompt::PlayTurn;
                self.current_player = state.offerer;
                Ok(())
            }
            ActionType::ConfirmTrade => {
                let partner = match action.payload {
                    ActionPayload::Trade { partner, .. } => partner
                        .ok_or(GameError::InvalidPayload("confirm trade requires partner"))?,
                    _ => {
                        return Err(GameError::InvalidPayload(
                            "confirm trade requires partner payload",
                        ));
                    }
                };
                if !self
                    .trade_state
                    .as_ref()
                    .map_or(false, |ts| ts.acceptees.contains(&partner))
                {
                    return Err(GameError::IllegalAction);
                }
                if !self.players[state.offerer]
                    .resources
                    .can_afford(&state.give)
                {
                    return Err(GameError::InsufficientResources);
                }
                if !self.players[partner].resources.can_afford(&state.receive) {
                    return Err(GameError::InsufficientResources);
                }
                self.players[state.offerer]
                    .remove_resources(&state.give)
                    .map_err(|_| GameError::InsufficientResources)?;
                self.players[partner]
                    .remove_resources(&state.receive)
                    .map_err(|_| GameError::InsufficientResources)?;
                self.players[state.offerer].add_resources(&state.receive);
                self.players[partner].add_resources(&state.give);
                self.clear_trade_state();
                self.pending_prompt = ActionPrompt::PlayTurn;
                self.current_player = state.offerer;
                Ok(())
            }
            _ => Err(GameError::IllegalAction),
        }
    }

    fn begin_discard_phase(&mut self) {
        self.discard_queue.clear();
        self.discard_targets.clear();
        for idx in 0..self.players.len() {
            let total = self.players[idx].resources.total() as u8;
            if total > 7 {
                let to_discard = total / 2;
                self.discard_queue.push_back(idx);
                self.discard_targets.insert(idx, to_discard);
            }
        }
        if let Some(next) = self.discard_queue.pop_front() {
            self.pending_prompt = ActionPrompt::Discard;
            self.current_player = next;
        } else {
            self.pending_prompt = ActionPrompt::MoveRobber;
            self.current_player = self.turn_owner;
        }
    }

    fn random_discard_bundle(
        &mut self,
        player_idx: usize,
        required: u8,
    ) -> Result<ResourceBundle, GameError> {
        if required == 0 {
            return Ok(ResourceBundle::zero());
        }
        let mut temp_counts = self.players[player_idx].resources.counts();
        let mut bundle = ResourceBundle::zero();
        for _ in 0..required {
            let mut bag = Vec::new();
            for (idx, &amount) in temp_counts.iter().enumerate() {
                let resource = Resource::ALL[idx];
                for _ in 0..amount {
                    bag.push(resource);
                }
            }
            if bag.is_empty() {
                break;
            }
            let choice = bag[self.rng.gen_range(0..bag.len())];
            bundle.add(choice, 1);
            let idx = resource_index(choice);
            temp_counts[idx] = temp_counts[idx].saturating_sub(1);
        }
        Ok(bundle)
    }

    fn steal_random_resource(&mut self, player_idx: usize) -> Option<Resource> {
        let mut bag = Vec::new();
        for (resource, amount) in self.players[player_idx].resources.iter() {
            for _ in 0..amount {
                bag.push(resource);
            }
        }
        if bag.is_empty() {
            return None;
        }
        let choice = bag[self.rng.gen_range(0..bag.len())];
        self.players[player_idx]
            .resources
            .subtract(choice, 1)
            .ok()?;
        Some(choice)
    }

    fn ensure_can_act_after_roll(&self) -> Result<(), GameError> {
        if self.awaiting_roll {
            Err(GameError::IllegalAction)
        } else {
            Ok(())
        }
    }

    fn pay_cost(&mut self, player_idx: usize, cost: &ResourceBundle) -> Result<(), GameError> {
        self.players[player_idx]
            .remove_resources(cost)
            .map_err(|_| GameError::InsufficientResources)?;
        self.bank.receive(cost);
        Ok(())
    }

    fn place_settlement(&mut self, player_idx: usize, node_id: NodeId) -> Result<(), GameError> {
        if self.node_occupancy.contains_key(&node_id) {
            return Err(GameError::NodeOccupied(node_id));
        }
        if let Some(neighbors) = self.map.node_neighbors.get(&node_id) {
            for neighbor in neighbors {
                if self.node_occupancy.contains_key(neighbor) {
                    return Err(GameError::DistanceRuleViolation);
                }
            }
        }
        self.players[player_idx].settlements.insert(node_id);
        self.node_occupancy
            .insert(node_id, Structure::Settlement { player: player_idx });
        Ok(())
    }

    fn upgrade_settlement_to_city(
        &mut self,
        player_idx: usize,
        node_id: NodeId,
    ) -> Result<(), GameError> {
        if self.players[player_idx].city_limit_reached() {
            return Err(GameError::IllegalAction);
        }
        if !self.players[player_idx].settlements.contains(&node_id) {
            return Err(GameError::IllegalAction);
        }
        self.pay_cost(player_idx, &COST_CITY)?;
        self.players[player_idx].settlements.remove(&node_id);
        self.players[player_idx].cities.insert(node_id);
        self.node_occupancy
            .insert(node_id, Structure::City { player: player_idx });
        Ok(())
    }

    fn place_road(&mut self, player_idx: usize, edge: EdgeId) {
        let normalized = normalize_edge(edge);
        self.players[player_idx].roads.insert(normalized);
        self.road_occupancy.insert(normalized, player_idx);
        self.update_longest_road();
    }

    fn award_starting_resources(
        &mut self,
        player_idx: usize,
        node_id: NodeId,
        outcome: &mut StepOutcome,
    ) -> Result<(), GameError> {
        let mut bundle = ResourceBundle::zero();
        if let Some(tile_ids) = self.map.adjacent_tiles.get(&node_id) {
            for tile_id in tile_ids {
                if let Some(tile) = self.map.tiles_by_id.get(tile_id) {
                    if let Some(resource) = tile.resource {
                        bundle.add(resource, 1);
                    }
                }
            }
        }
        if !bundle.is_empty() {
            if self.bank.dispense(&bundle).is_ok() {
                self.players[player_idx].add_resources(&bundle);
                outcome.events.push(GameEvent::ResourcesDistributed {
                    player: player_idx,
                    bundle,
                });
            }
        }
        Ok(())
    }

    fn roll_die(&mut self) -> u8 {
        self.rng.gen_range(1..=6)
    }

    fn distribute_resources(
        &mut self,
        dice_sum: u8,
        outcome: &mut StepOutcome,
    ) -> Result<(), GameError> {
        for tile in self.map.tiles_by_id.values() {
            if tile.number != Some(dice_sum) {
                continue;
            }
            if tile.id == self.robber_tile {
                continue;
            }

            for (_node_ref, node_id) in &tile.nodes {
                if let Some(structure) = self.node_occupancy.get(node_id) {
                    let multiplier = match structure {
                        Structure::Settlement { .. } => 1,
                        Structure::City { .. } => 2,
                    };
                    if let Some(resource) = tile.resource {
                        let mut bundle = ResourceBundle::zero();
                        bundle.add(resource, multiplier);
                        let owner = match structure {
                            Structure::Settlement { player } => *player,
                            Structure::City { player } => *player,
                        };
                        if self.bank.dispense(&bundle).is_ok() {
                            self.players[owner].add_resources(&bundle);
                            outcome.events.push(GameEvent::ResourcesDistributed {
                                player: owner,
                                bundle,
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_settlement_location(
        &self,
        player_idx: usize,
        node_id: NodeId,
        require_network: bool,
    ) -> Result<(), GameError> {
        if self.players[player_idx].settlement_limit_reached() {
            return Err(GameError::IllegalAction);
        }
        if self.node_occupancy.contains_key(&node_id) {
            return Err(GameError::NodeOccupied(node_id));
        }
        if let Some(neighbors) = self.map.node_neighbors.get(&node_id) {
            for neighbor in neighbors {
                if self.node_occupancy.contains_key(neighbor) {
                    return Err(GameError::DistanceRuleViolation);
                }
            }
        }
        if require_network && !self.node_connected_to_player_network(player_idx, node_id) {
            return Err(GameError::MustConnectToNetwork);
        }
        Ok(())
    }

    fn validate_road_location(
        &self,
        player_idx: usize,
        edge: EdgeId,
        require_network: bool,
    ) -> Result<(), GameError> {
        if self.players[player_idx].road_limit_reached() {
            return Err(GameError::IllegalAction);
        }
        let normalized = normalize_edge(edge);
        if self.road_occupancy.contains_key(&normalized) {
            return Err(GameError::EdgeOccupied);
        }
        let node_a = normalized.0;
        let node_b = normalized.1;
        if !self
            .map
            .node_neighbors
            .get(&node_a)
            .map_or(false, |neighbors| neighbors.contains(&node_b))
        {
            return Err(GameError::EdgeNotFound);
        }
        if require_network {
            let connected = self.players[player_idx].roads.iter().any(|existing| {
                let nodes = [existing.0, existing.1];
                nodes.contains(&node_a) || nodes.contains(&node_b)
            }) || self.players[player_idx].settlements.contains(&node_a)
                || self.players[player_idx].settlements.contains(&node_b)
                || self.players[player_idx].cities.contains(&node_a)
                || self.players[player_idx].cities.contains(&node_b);
            if !connected {
                return Err(GameError::MustConnectToNetwork);
            }
        }
        Ok(())
    }

    fn node_connected_to_player_network(&self, player_idx: usize, node_id: NodeId) -> bool {
        self.players[player_idx]
            .roads
            .iter()
            .any(|edge| edge_contains_node(*edge, node_id))
            || self.players[player_idx].settlements.contains(&node_id)
            || self.players[player_idx].cities.contains(&node_id)
    }

    fn advance_turn(&mut self, outcome: &mut StepOutcome) {
        self.clear_road_building();
        let finished = self.current_player;
        if let Some(player) = self.players.get_mut(finished) {
            player.reset_for_new_turn();
        }
        self.current_player = (self.current_player + 1) % self.players.len();
        self.turn_owner = self.current_player;
        self.turn += 1;
        self.awaiting_roll = true;
        self.pending_prompt = ActionPrompt::PlayTurn;
        outcome.events.push(GameEvent::TurnAdvanced {
            next_player: self.current_player,
        });
    }

    fn check_victory(&mut self) {
        if matches!(self.phase, GamePhase::Completed { .. }) {
            return;
        }
        for (idx, player) in self.players.iter().enumerate() {
            if player.total_points() >= self.config.vps_to_win {
                self.phase = GamePhase::Completed { winner: Some(idx) };
                break;
            }
        }
    }
}

impl GameState {
    pub fn legal_actions(&self) -> &[GameAction] {
        &self.available_actions
    }

    pub fn action_log(&self) -> &[GameAction] {
        &self.actions
    }

    fn legal_setup_actions(&self, state: &SetupState) -> Vec<GameAction> {
        let mut actions = Vec::new();
        let Some(player_idx) = state.current_player() else {
            return actions;
        };
        let prompt = state.current_prompt().unwrap_or(ActionPrompt::PlayTurn);
        match prompt {
            ActionPrompt::BuildInitialSettlement => {
                for node in &self.map.land_nodes {
                    if self
                        .validate_settlement_location(player_idx, *node, false)
                        .is_ok()
                    {
                        actions.push(
                            GameAction::new(player_idx, ActionType::BuildSettlement)
                                .with_payload(ActionPayload::Node(*node)),
                        );
                    }
                }
            }
            ActionPrompt::BuildInitialRoad => {
                if let Some(&anchor) = self.setup_pending_roads.get(&player_idx) {
                    for edge in self.unique_edges() {
                        if edge.0 != anchor && edge.1 != anchor {
                            continue;
                        }
                        if self.validate_road_location(player_idx, edge, false).is_ok() {
                            actions.push(
                                GameAction::new(player_idx, ActionType::BuildRoad)
                                    .with_payload(ActionPayload::Edge(edge)),
                            );
                        }
                    }
                }
            }
            _ => {}
        }
        actions
    }

    fn legal_play_actions(&self) -> Vec<GameAction> {
        match self.pending_prompt {
            ActionPrompt::PlayTurn => self.legal_play_turn_actions(),
            ActionPrompt::Discard => self.legal_discard_actions(),
            ActionPrompt::MoveRobber => self.legal_move_robber_actions(),
            ActionPrompt::DecideTrade => self.legal_trade_response_actions(),
            ActionPrompt::DecideAcceptees => self.legal_trade_confirmation_actions(),
            _ => Vec::new(),
        }
    }

    fn legal_play_turn_actions(&self) -> Vec<GameAction> {
        if matches!(self.phase, GamePhase::Completed { .. }) {
            return Vec::new();
        }
        let mut actions = Vec::new();
        if self.awaiting_roll {
            actions.push(GameAction::new(self.current_player, ActionType::Roll));
        } else {
            actions.push(GameAction::new(self.current_player, ActionType::EndTurn));
        }
        let player_idx = self.current_player;
        let player = &self.players[player_idx];
        
        let is_road_building = !player.road_limit_reached() && self.road_building_player == Some(player_idx) && self.road_building_free_roads > 0;
        if is_road_building {
            for edge in self.unique_edges() {
                if self.validate_road_location(player_idx, edge, true).is_ok() {
                    actions.push(
                        GameAction::new(player_idx, ActionType::BuildRoad)
                            .with_payload(ActionPayload::Edge(edge)),
                    );
                }
            }
        }

        if !self.awaiting_roll {
            if !is_road_building && !player.road_limit_reached() && player.resources.can_afford(&COST_ROAD) {
                for edge in self.unique_edges() {
                    if self.validate_road_location(player_idx, edge, true).is_ok() {
                        actions.push(
                            GameAction::new(player_idx, ActionType::BuildRoad)
                                .with_payload(ActionPayload::Edge(edge)),
                        );
                    }
                }
            }

            if !player.settlement_limit_reached() && player.resources.can_afford(&COST_SETTLEMENT) {
                for node in &self.map.land_nodes {
                    if self
                        .validate_settlement_location(player_idx, *node, true)
                        .is_ok()
                    {
                        actions.push(
                            GameAction::new(player_idx, ActionType::BuildSettlement)
                                .with_payload(ActionPayload::Node(*node)),
                        );
                    }
                }
            }

            if !player.city_limit_reached() && player.resources.can_afford(&COST_CITY) {
                for node in &player.settlements {
                    actions.push(
                        GameAction::new(player_idx, ActionType::BuildCity)
                            .with_payload(ActionPayload::Node(*node)),
                    );
                }
            }

            if self.bank.development_deck_len() > 0
                && player.resources.can_afford(&COST_DEVELOPMENT)
            {
                actions.push(GameAction::new(player_idx, ActionType::BuyDevelopmentCard));
            }

            actions.extend(self.legal_maritime_trades(player_idx));
        }

        actions.extend(self.legal_dev_card_actions(player_idx));

        actions
    }

    fn legal_discard_actions(&self) -> Vec<GameAction> {
        let mut actions = Vec::new();
        let player_resources = self.players[self.current_player].resources;
        for (resource, count) in player_resources.iter() {
            if count > 0 {
                actions.push(
                    GameAction::new(self.current_player, ActionType::Discard).
                    with_payload(ActionPayload::Resource(resource))
                );
            }
        }

        actions
    }

    fn legal_move_robber_actions(&self) -> Vec<GameAction> {
        let mut actions = Vec::new();
        for tile in self.map.tiles_by_id.values() {
            if tile.id == self.robber_tile {
                continue;
            }
            let mut victims = HashSet::new();
            for node_id in tile.nodes.values() {
                if let Some(structure) = self.node_occupancy.get(node_id) {
                    let owner = match structure {
                        Structure::Settlement { player } | Structure::City { player } => *player,
                    };
                    if owner != self.current_player && !self.players[owner].resources.is_empty() {
                        victims.insert(owner);
                    }
                }
            }
            if victims.is_empty() {
                actions.push(
                    GameAction::new(self.current_player, ActionType::MoveRobber).with_payload(
                        ActionPayload::Robber {
                            tile_id: tile.id,
                            victim: None,
                            resource: None,
                        },
                    ),
                );
            } else {
                for victim in victims {
                    actions.push(
                        GameAction::new(self.current_player, ActionType::MoveRobber).with_payload(
                            ActionPayload::Robber {
                                tile_id: tile.id,
                                victim: Some(victim),
                                resource: None,
                            },
                        ),
                    );
                }
            }
        }
        actions
    }

    fn legal_trade_response_actions(&self) -> Vec<GameAction> {
        let Some(state) = &self.trade_state else {
            return Vec::new();
        };
        if self.current_player == state.offerer {
            return Vec::new();
        }
        let mut actions = vec![GameAction::new(
            self.current_player,
            ActionType::RejectTrade,
        )];
        if self.players[self.current_player]
            .resources
            .can_afford(&state.receive)
        {
            actions.push(GameAction::new(
                self.current_player,
                ActionType::AcceptTrade,
            ));
        }
        actions
    }

    fn legal_trade_confirmation_actions(&self) -> Vec<GameAction> {
        let Some(state) = &self.trade_state else {
            return Vec::new();
        };
        if self.current_player != state.offerer {
            return Vec::new();
        }
        let mut actions = vec![GameAction::new(
            self.current_player,
            ActionType::CancelTrade,
        )];
        for partner in &state.acceptees {
            actions.push(
                GameAction::new(self.current_player, ActionType::ConfirmTrade).with_payload(
                    ActionPayload::Trade {
                        give: state.give,
                        receive: state.receive,
                        partner: Some(*partner),
                    },
                ),
            );
        }
        actions
    }

    fn legal_maritime_trades(&self, player_idx: usize) -> Vec<GameAction> {
        let mut actions = Vec::new();
        for resource in Resource::ALL {
            let available = self.players[player_idx].resources.get(resource);
            if available == 0 {
                continue;
            }
            let rate = self.maritime_rate(player_idx, resource);
            if available < rate {
                continue;
            }
            for target in Resource::ALL {
                if target == resource {
                    continue;
                }
                if self.bank.available(target) == 0 {
                    continue;
                }
                let mut give = ResourceBundle::zero();
                give.add(resource, rate);
                actions.push(
                    GameAction::new(player_idx, ActionType::MaritimeTrade).with_payload(
                        ActionPayload::MaritimeTrade {
                            give,
                            receive: target,
                        },
                    ),
                );
            }
        }
        actions
    }

    fn legal_dev_card_actions(&self, player_idx: usize) -> Vec<GameAction> {
        let player = &self.players[player_idx];
        if player.has_played_dev_card_this_turn {
            return Vec::new();
        }
        let mut actions = Vec::new();
        if player.can_play_dev_card(DevelopmentCard::Knight) {
            actions.push(
                GameAction::new(player_idx, ActionType::PlayKnightCard)
                    .with_payload(ActionPayload::None),
            );
        }
        if player.can_play_dev_card(DevelopmentCard::YearOfPlenty) {
            actions.extend(self.year_of_plenty_actions(player_idx));
        }
        if player.can_play_dev_card(DevelopmentCard::Monopoly) {
            for resource in Resource::ALL {
                actions.push(
                    GameAction::new(player_idx, ActionType::PlayMonopoly)
                        .with_payload(ActionPayload::Resource(resource)),
                );
            }
        }
        if player.can_play_dev_card(DevelopmentCard::RoadBuilding) {
            actions.push(
                GameAction::new(player_idx, ActionType::PlayRoadBuilding)
                    .with_payload(ActionPayload::None),
            );
        }
        actions
    }

    fn year_of_plenty_actions(&self, player_idx: usize) -> Vec<GameAction> {
        let mut actions = Vec::new();
        for (i, resource) in Resource::ALL.iter().enumerate() {
            if self.bank.available(*resource) == 0 {
                continue;
            }
            let mut single = ResourceBundle::zero();
            single.add(*resource, 1);
            actions.push(
                GameAction::new(player_idx, ActionType::PlayYearOfPlenty)
                    .with_payload(ActionPayload::Resources(single)),
            );
            for resource_b in Resource::ALL.iter().skip(i) {
                let mut bundle = ResourceBundle::zero();
                bundle.add(*resource, 1);
                if resource == resource_b {
                    if self.bank.available(*resource) < 2 {
                        continue;
                    }
                    bundle.add(*resource, 1);
                } else {
                    if self.bank.available(*resource_b) == 0 {
                        continue;
                    }
                    bundle.add(*resource_b, 1);
                }
                actions.push(
                    GameAction::new(player_idx, ActionType::PlayYearOfPlenty)
                        .with_payload(ActionPayload::Resources(bundle)),
                );
            }
        }
        actions
    }

    fn refresh_available_actions(&mut self) {
        self.available_actions = self.compute_available_actions();
    }

    fn compute_available_actions(&self) -> Vec<GameAction> {
        match &self.phase {
            GamePhase::Setup(state) => self.legal_setup_actions(state),
            GamePhase::Playing => self.legal_play_actions(),
            GamePhase::Completed { .. } => Vec::new(),
        }
    }

    fn unique_edges(&self) -> Vec<EdgeId> {
        let mut seen = HashSet::new();
        let mut edges = Vec::new();
        for list in self.map.node_edges.values() {
            for edge in list {
                let normalized = normalize_edge(*edge);
                if seen.insert(normalized) {
                    edges.push(normalized);
                }
            }
        }
        edges
    }

    fn update_longest_road(&mut self) {
        let mut best_len = 0;
        let mut best_idx: Option<usize> = None;
        let mut tie = false;
        for idx in 0..self.players.len() {
            let len = self.player_longest_road(idx);
            if len < 5 {
                continue;
            }
            if len > best_len {
                best_len = len;
                best_idx = Some(idx);
                tie = false;
            } else if len == best_len {
                tie = true;
            }
        }
        for (idx, player) in self.players.iter_mut().enumerate() {
            player.has_longest_road = best_idx == Some(idx) && !tie && best_len >= 5;
        }
    }

    fn player_longest_road(&self, player_idx: usize) -> usize {
        let player = &self.players[player_idx];
        if player.roads.is_empty() {
            return 0;
        }
        let blocked = self.blocked_nodes(player_idx);
        let mut best = 0;
        for &(a, b) in &player.roads {
            best = best.max(self.longest_from_node(player_idx, a, &blocked, &mut HashSet::new()));
            best = best.max(self.longest_from_node(player_idx, b, &blocked, &mut HashSet::new()));
        }
        best
    }

    fn longest_from_node(
        &self,
        player_idx: usize,
        start: NodeId,
        blocked: &HashSet<NodeId>,
        visited_edges: &mut HashSet<EdgeId>,
    ) -> usize {
        let mut best = 0;
        if let Some(neighbors) = self.map.node_neighbors.get(&start) {
            for &neighbor in neighbors {
                if blocked.contains(&neighbor) && !self.node_owned_by(player_idx, neighbor) {
                    continue;
                }
                let edge = normalize_edge((start, neighbor));
                if visited_edges.contains(&edge) {
                    continue;
                }
                if !self.players[player_idx].roads.contains(&edge) {
                    continue;
                }
                visited_edges.insert(edge);
                let depth =
                    1 + self.longest_from_node(player_idx, neighbor, blocked, visited_edges);
                visited_edges.remove(&edge);
                best = best.max(depth);
            }
        }
        best
    }

    fn blocked_nodes(&self, player_idx: usize) -> HashSet<NodeId> {
        self.node_occupancy
            .iter()
            .filter_map(|(node, structure)| match structure {
                Structure::Settlement { player } | Structure::City { player } => {
                    if *player == player_idx {
                        None
                    } else {
                        Some(*node)
                    }
                }
            })
            .collect()
    }

    fn update_largest_army(&mut self) {
        let mut best_idx: Option<usize> = None;
        let mut best_size = 0;
        let mut tie = false;
        for (idx, player) in self.players.iter().enumerate() {
            if player.knights_played < 3 {
                continue;
            }
            if player.knights_played > best_size {
                best_size = player.knights_played;
                best_idx = Some(idx);
                tie = false;
            } else if player.knights_played == best_size {
                tie = true;
            }
        }
        for (idx, player) in self.players.iter_mut().enumerate() {
            player.has_largest_army = best_idx == Some(idx) && !tie && best_size >= 3;
        }
    }
}

fn normalize_edge(edge: EdgeId) -> EdgeId {
    if edge.0 <= edge.1 {
        edge
    } else {
        (edge.1, edge.0)
    }
}

fn edge_contains_node(edge: EdgeId, node: NodeId) -> bool {
    edge.0 == node || edge.1 == node
}

fn resource_index(resource: Resource) -> usize {
    match resource {
        Resource::Wood => 0,
        Resource::Brick => 1,
        Resource::Sheep => 2,
        Resource::Wheat => 3,
        Resource::Ore => 4,
    }
}

#[derive(Debug, Clone)]
pub struct SetupState {
    steps: Vec<SetupStep>,
    cursor: usize,
}

#[derive(Debug, Clone)]
struct SetupStep {
    player_index: usize,
    prompt: ActionPrompt,
    second_round: bool,
}

impl SetupState {
    fn new(num_players: usize) -> Self {
        let mut steps = Vec::with_capacity(num_players * 4);
        for player in 0..num_players {
            steps.push(SetupStep {
                player_index: player,
                prompt: ActionPrompt::BuildInitialSettlement,
                second_round: false,
            });
            steps.push(SetupStep {
                player_index: player,
                prompt: ActionPrompt::BuildInitialRoad,
                second_round: false,
            });
        }

        for player in (0..num_players).rev() {
            steps.push(SetupStep {
                player_index: player,
                prompt: ActionPrompt::BuildInitialSettlement,
                second_round: true,
            });
            steps.push(SetupStep {
                player_index: player,
                prompt: ActionPrompt::BuildInitialRoad,
                second_round: true,
            });
        }

        Self { steps, cursor: 0 }
    }

    fn current_prompt(&self) -> Option<ActionPrompt> {
        self.steps.get(self.cursor).map(|step| step.prompt)
    }

    fn current_player(&self) -> Option<usize> {
        self.steps.get(self.cursor).map(|step| step.player_index)
    }

    fn is_second_settlement(&self) -> bool {
        self.steps
            .get(self.cursor)
            .map(|step| {
                step.second_round && matches!(step.prompt, ActionPrompt::BuildInitialSettlement)
            })
            .unwrap_or(false)
    }

    fn advance(&mut self) {
        if self.cursor < self.steps.len() {
            self.cursor += 1;
        }
    }

    fn is_complete(&self) -> bool {
        self.cursor >= self.steps.len()
    }
}
