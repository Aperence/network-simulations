use std::sync::Arc;
use tokio::sync::Mutex;

pub type SharedState<V> = Arc<Mutex<V>>;

#[derive(Debug, Clone, PartialEq)]
pub struct MacAddress{
    pub id: u32 // for simplicity, we simply use an int as an address
}