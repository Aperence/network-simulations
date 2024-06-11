
pub mod network;

use std::{net::Ipv4Addr, thread, time::Duration};

use network::logger::Source;

use self::network::Network;

#[tokio::main]
async fn main() -> Result<(), ()> {
    env_logger::init(); // you can run using `RUST_LOG=debug cargo run` to get more details on what messages the switches are exchanging

    let mut network = Network::new_with_filters(vec![Source::Ping]);
    network.add_router("r1".into(), 1);
    network.add_router("r2".into(), 2);
    network.add_switch("s1".into(), 11);
    network.add_switch("s2".into(), 12);
    network.add_switch("s3".into(), 13);

    network.add_link("r1".into(), 1, "s1".into(), 1, 1).await;
    network.add_link("s1".into(), 2, "s2".into(), 1, 1).await;
    network.add_link("s2".into(), 2, "s3".into(), 1, 1).await;
    network.add_link("s3".into(), 2, "r2".into(), 1, 1).await;

    // wait for convergence
    thread::sleep(Duration::from_millis(500));

    network.ping("r1".into(), Ipv4Addr::new(10, 0, 0, 2)).await;

    thread::sleep(Duration::from_millis(500));

    network.quit().await;

    Ok(())
}
