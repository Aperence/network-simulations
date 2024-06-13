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
    network.add_router("r5", 5, 5);
    network.add_router("r6", 6, 6);
    network.add_router("r7", 7, 7);
    network.add_router("r8", 8, 8);


    network.add_provider_customer_link("r3", 1, "r1", 1, 0).await;
    network.add_provider_customer_link("r1", 2, "r2", 1, 0).await;
    network.add_provider_customer_link("r4", 1, "r3", 3, 0).await;
    network.add_provider_customer_link("r5", 1, "r2", 3, 0).await;
    network.add_provider_customer_link("r7", 1, "r4", 3, 0).await;
    network.add_provider_customer_link("r6", 2, "r7", 2, 0).await;
    network.add_provider_customer_link("r8", 1, "r7", 3, 0).await;

    network.add_peer_link("r2", 2, "r3", 2, 0).await;
    network.add_peer_link("r4", 2, "r5", 2, 0).await;
    network.add_peer_link("r5", 3, "r6", 1, 0).await;
    network.add_peer_link("r6", 3, "r8", 2, 0).await;


    network.announce_prefix("r2").await;

    // wait for convergence
    thread::sleep(Duration::from_millis(2000));

    network.print_bgp_tables().await;

    network.quit().await;

    Ok(())
}
