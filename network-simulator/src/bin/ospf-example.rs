
use std::{thread, time::Duration};

use network_simulator::network::logger::Source;

use network_simulator::network::Network;

#[tokio::main]
async fn main() -> Result<(), ()> {
    env_logger::init(); // you can run using `RUST_LOG=debug cargo run` to get more details on what messages the switches are exchanging

    let mut network = Network::new_with_filters(vec![Source::Ping, Source::OSPF]);
    network.add_router("r1", 1, 1);
    network.add_router("r2", 2, 1);
    network.add_router("r3", 3, 1);
    network.add_router("r4", 4, 1);

    network.add_link("r1", 1, "r2", 1, 1).await;
    network.add_link("r1", 2, "r3", 1, 1).await;
    network.add_link("r3", 3, "r4", 1, 1).await;
    network.add_link("r2", 2, "r3", 2, 1).await;

    // wait for convergence
    thread::sleep(Duration::from_millis(1000));

    network.print_routing_tables().await;
    network.ping("r1", "10.0.1.4".parse().unwrap()).await;

    thread::sleep(Duration::from_millis(1000));

    network.quit().await;

    Ok(())
}
