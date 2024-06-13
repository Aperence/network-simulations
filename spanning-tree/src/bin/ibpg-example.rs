use std::{thread, time::Duration};

use network_simulator::network::logger::Source;

use network_simulator::network::Network;

#[tokio::main]
async fn main() -> Result<(), ()> {
    env_logger::init(); // you can run using `RUST_LOG=debug cargo run` to get more details on what messages the switches are exchanging

    let mut network = Network::new_with_filters(vec![Source::BGP, Source::Ping]);
    network.add_router("r1".into(), 1, 1);
    network.add_router("r2".into(), 2, 1);
    network.add_router("r3".into(), 3, 1);
    network.add_router("r4".into(), 4, 2);
    network.add_router("r5".into(), 5, 3);

    network
        .add_provider_customer_link("r4".into(), 1, "r1".into(), 1, 0)
        .await;

    network
        .add_provider_customer_link("r3".into(), 1, "r5".into(), 3, 0)
        .await;

    network
        .add_link("r1".into(), 2, "r2".into(), 1, 0)
        .await;
    network
        .add_link("r2".into(), 2, "r3".into(), 1, 0)
        .await;
    network
        .add_link("r1".into(), 3, "r3".into(), 2, 0)
        .await;

    let routers = ["r1", "r2", "r3"];
    for i in 0..routers.len(){
        for j in i+1..routers.len(){
            network.add_ibgp_connection(routers[i].into(), routers[j].into()).await;
        }
    }

    // wait for convergence
    thread::sleep(Duration::from_millis(250));

    network.print_routing_tables().await;

    network.announce_prefix("r4".into()).await;
    network.announce_prefix("r5".into()).await;

    thread::sleep(Duration::from_millis(500));

    network.print_bgp_tables().await;
    network.ping("r4".into(), "10.0.3.5".parse().unwrap()).await;

    thread::sleep(Duration::from_millis(1000));

    network.quit().await;

    Ok(())
}
