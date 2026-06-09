use crate::models::{CommunitySummary, RoomRecommendation};

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomExplorerSnapshot {
    pub featured: Vec<CommunitySummary>,
    pub new_noteworthy: Vec<CommunitySummary>,
    pub friends_shelf: Vec<RoomRecommendation>,
    pub authors_shelf: Vec<RoomRecommendation>,
}
