use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    EnumString,
    Display,
    EnumIter,
)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum Resource {
    Wood,
    Brick,
    Sheep,
    Wheat,
    Ore,
}

impl Resource {
    pub const ALL: [Resource; 5] = [
        Resource::Wood,
        Resource::Brick,
        Resource::Sheep,
        Resource::Wheat,
        Resource::Ore,
    ];
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumString, Display, EnumIter,
)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum DevelopmentCard {
    Knight,
    YearOfPlenty,
    Monopoly,
    RoadBuilding,
    VictoryPoint,
}

impl DevelopmentCard {
    pub const ALL: [DevelopmentCard; 5] = [
        DevelopmentCard::Knight,
        DevelopmentCard::YearOfPlenty,
        DevelopmentCard::Monopoly,
        DevelopmentCard::RoadBuilding,
        DevelopmentCard::VictoryPoint,
    ];
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumString, Display, EnumIter,
)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum BuildingKind {
    Settlement,
    City,
    Road,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumString, Display, EnumIter,
)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum Color {
    Red,
    Blue,
    Orange,
    White,
}

impl Color {
    pub const ORDERED: [Color; 4] = [Color::Red, Color::Blue, Color::Orange, Color::White];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter)]
pub enum NodeRef {
    North,
    NorthEast,
    SouthEast,
    South,
    SouthWest,
    NorthWest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumIter)]
pub enum EdgeRef {
    East,
    SouthEast,
    SouthWest,
    West,
    NorthWest,
    NorthEast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumString, Display)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionPrompt {
    BuildInitialSettlement,
    BuildInitialRoad,
    PlayTurn,
    Discard,
    MoveRobber,
    DecideTrade,
    DecideAcceptees,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumString, Display)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionType {
    Roll,
    MoveRobber,
    Discard,
    BuildRoad,
    BuildSettlement,
    BuildCity,
    BuyDevelopmentCard,
    PlayKnightCard,
    PlayYearOfPlenty,
    PlayMonopoly,
    PlayRoadBuilding,
    MaritimeTrade,
    OfferTrade,
    AcceptTrade,
    RejectTrade,
    ConfirmTrade,
    CancelTrade,
    EndTurn,
}
