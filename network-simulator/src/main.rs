
pub mod network;

use std::{thread, time::Duration};

use network::logger::Source;

use self::network::Network;

#[tokio::main]
async fn main() -> Result<(), ()> {
    env_logger::init(); // you can run using `RUST_LOG=debug cargo run` to get more details on what messages the switches are exchanging

    let mut network = Network::new_with_filters(vec![Source::BGP, Source::Ping, Source::IP]);
    network.add_router("r1", 1, 1);
    network.add_router("r2", 2, 1);
    network.add_router("r3", 3, 1);
    network.add_router("r4", 4, 2);
    network.add_router("r5", 5, 3);

    network
        .add_provider_customer_link("r4", 1, "r1", 1, 0)
        .await;

    network
        .add_provider_customer_link("r3", 1, "r5", 3, 0)
        .await;

    network
        .add_link("r1", 2, "r2", 1, 0)
        .await;
    network
        .add_link("r2", 2, "r3", 3, 0)
        .await;
    network
        .add_link("r1", 3, "r3", 2, 0)
        .await;

    let routers = ["r1", "r2", "r3"];
    for i in 0..routers.len(){
        for j in i+1..routers.len(){
            network.add_ibgp_connection(routers[i].into(), routers[j].into()).await;
        }
    }

    // wait for convergence
    thread::sleep(Duration::from_millis(1000));

    network.announce_prefix("r4").await;
    network.announce_prefix("r5").await;

    thread::sleep(Duration::from_millis(1000));

    network.print_bgp_tables().await;

    network.quit().await;

    Ok(())
}
