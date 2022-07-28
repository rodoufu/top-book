#[derive(Serialize, Deserialize)]
enum OrderbookResponse {
    Snapshot,
    Update,
}