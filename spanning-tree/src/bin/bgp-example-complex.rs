use std::{thread, time::Duration};

use network_simulator::network::logger::Source;

use network_simulator::network::Network;

#[tokio::main]
async fn main() -> Result<(), ()> {
    env_logger::init(); // you can run using `RUST_LOG=debug cargo run` to get more details on what messages the switches are exchanging

    let mut network = Network::new_with_filters(vec![Source::Ping, Source::BGP]);
    network.add_router("r1".into(), 1, 1);
    network.add_router("r2".into(), 2, 2);
    network.add_router("r3".into(), 3, 3);
    network.add_router("r4".into(), 4, 4);
    network.add_router("r5".into(), 5, 5);
    network.add_router("r6".into(), 6, 6);
    network.add_router("r7".into(), 7, 7);
    network.add_router("r8".into(), 8, 8);


    network.add_provider_customer_link("r3".into(), 1, "r1".into(), 1, 0).await;
    network.add_provider_customer_link("r1".into(), 2, "r2".into(), 1, 0).await;
    network.add_provider_customer_link("r4".into(), 1, "r3".into(), 3, 0).await;
    network.add_provider_customer_link("r5".into(), 1, "r2".into(), 3, 0).await;
    network.add_provider_customer_link("r7".into(), 1, "r4".into(), 3, 0).await;
    network.add_provider_customer_link("r6".into(), 2, "r7".into(), 2, 0).await;
    network.add_provider_customer_link("r8".into(), 1, "r7".into(), 3, 0).await;

    network.add_peer_link("r2".into(), 2, "r3".into(), 2, 0).await;
    network.add_peer_link("r4".into(), 2, "r5".into(), 2, 0).await;
    network.add_peer_link("r5".into(), 3, "r6".into(), 1, 0).await;
    network.add_peer_link("r6".into(), 3, "r8".into(), 2, 0).await;


    network.announce_prefix("r2".into()).await;

    // wait for convergence
    thread::sleep(Duration::from_millis(2000));

    network.print_bgp_tables().await;

    network.quit().await;

    Ok(())
}
