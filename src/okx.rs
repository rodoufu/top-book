use serde_derive::{
    Deserialize,
    Serialize,
};

#[derive(Serialize, Deserialize)]
enum OrderbookResponse {
    Snapshot,
    Update,
}