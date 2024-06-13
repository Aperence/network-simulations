
use std::{thread, time::Duration};

use network_simulator::network::logger::Source;

use network_simulator::network::Network;

#[tokio::main]
async fn main() -> Result<(), ()> {
    env_logger::init(); // you can run using `RUST_LOG=debug cargo run` to get more details on what messages the switches are exchanging

    let mut network = Network::new_with_filters(vec![Source::Ping, Source::OSPF]);
    network.add_router("r1".into(), 1, 1);
    network.add_router("r2".into(), 2, 1);
    network.add_router("r3".into(), 3, 1);
    network.add_router("r4".into(), 4, 1);

    network.add_link("r1".into(), 1, "r2".into(), 1, 1).await;
    network.add_link("r1".into(), 2, "r3".into(), 1, 1).await;
    network.add_link("r3".into(), 3, "r4".into(), 1, 1).await;
    network.add_link("r2".into(), 2, "r3".into(), 2, 1).await;

    // wait for convergence
    thread::sleep(Duration::from_millis(250));

    network.print_routing_tables().await;
    network.ping("r1".into(), "10.0.1.4".parse().unwrap()).await;

    thread::sleep(Duration::from_millis(250));

    network.quit().await;

    Ok(())
}
