
use std::{thread, time::Duration};

use network_simulator::network::logger::Source;

use network_simulator::network::Network;

#[tokio::main]
async fn main() -> Result<(), ()> {
    env_logger::init(); // you can run using `RUST_LOG=debug cargo run` to get more details on what messages the switches are exchanging

    let mut network = Network::new_with_filters(vec![Source::Ping, Source::BGP]);
    network.add_router("r1", 1, 1);
    network.add_router("r2", 2, 2);
    network.add_router("r3", 3, 3);
    network.add_router("r4", 4, 4);

    network.add_provider_customer_link("r2", 1, "r1", 1, 0).await;
    network.add_provider_customer_link("r2", 2, "r4", 1, 0).await;
    network.add_provider_customer_link("r4", 3, "r3", 1, 0).await;

    network.add_peer_link("r1", 2, "r4", 2, 0).await;


    network.announce_prefix("r1").await;
    network.announce_prefix("r3").await;

    // wait for convergence
    thread::sleep(Duration::from_millis(1000));

    network.print_bgp_tables().await;
    network.ping("r1", "10.0.3.3".parse().unwrap()).await;

    thread::sleep(Duration::from_millis(1000));

    network.quit().await;

    Ok(())
}
