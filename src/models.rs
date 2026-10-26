use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Pagination {
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiResponse<T> {
    pub data: Vec<T>,
    #[serde(default)]
    pub pagination: Option<Pagination>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Stream {
    pub id: String,
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    #[serde(default)]
    pub game_id: String,
    #[serde(default)]
    pub game_name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub viewer_count: u64,
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub thumbnail_url: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub is_mature: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct User {
    pub id: String,
    pub login: String,
    pub display_name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub broadcaster_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub profile_image_url: String,
    #[serde(default)]
    pub offline_image_url: String,
    #[serde(default)]
    pub view_count: u64,
    #[serde(default)]
    pub created_at: String,
}
