
pub mod network;

use std::{thread, time::Duration};

use network::logger::Source;

use self::network::Network;

#[tokio::main]
async fn main() -> Result<(), ()> {
    env_logger::init(); // you can run using `RUST_LOG=debug cargo run` to get more details on what messages the switches are exchanging

    let mut network = Network::new_with_filters(vec![Source::BGP]);
    network.add_router("r1".into(), 1, 1);
    network.add_router("r2".into(), 2, 2);
    network.add_router("r3".into(), 3, 3);
    network.add_router("r4".into(), 4, 4);

    network.add_provider_customer_link("r2".into(), 1, "r1".into(), 1, 0).await;
    network.add_provider_customer_link("r2".into(), 2, "r4".into(), 1, 0).await;
    network.add_provider_customer_link("r4".into(), 3, "r3".into(), 1, 0).await;

    network.add_peer_link("r1".into(), 2, "r4".into(), 2, 0).await;


    network.announce_prefix("r1".into()).await;

    // wait for convergence
    thread::sleep(Duration::from_millis(250));

    network.quit().await;

    Ok(())
}
