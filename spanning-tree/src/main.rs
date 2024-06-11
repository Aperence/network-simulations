
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
    network.add_router("r3".into(), 3);
    network.add_router("r4".into(), 4);

    network.add_link("r1".into(), 1, "r2".into(), 1, 1).await;
    network.add_link("r1".into(), 2, "r3".into(), 1, 1).await;
    network.add_link("r3".into(), 3, "r4".into(), 1, 1).await;
    network.add_link("r2".into(), 2, "r3".into(), 2, 1).await;

    // wait for convergence
    thread::sleep(Duration::from_millis(1000));

    assert_eq!(network.get_routing_table("r1".into()).await, [
        (Ipv4Addr::new(10, 0, 0, 1), (0, 0)), 
        (Ipv4Addr::new(10, 0, 0, 2), (1, 1)), 
        (Ipv4Addr::new(10, 0, 0, 3), (2, 1)), 
        (Ipv4Addr::new(10, 0, 0, 4), (2, 2))
        ].into_iter().collect());

    assert_eq!(network.get_routing_table("r2".into()).await, [
        (Ipv4Addr::new(10, 0, 0, 1), (1, 1)), 
        (Ipv4Addr::new(10, 0, 0, 2), (0, 0)), 
        (Ipv4Addr::new(10, 0, 0, 3), (2, 1)), 
        (Ipv4Addr::new(10, 0, 0, 4), (2, 2))
        ].into_iter().collect());

    assert_eq!(network.get_routing_table("r3".into()).await, [
        (Ipv4Addr::new(10, 0, 0, 1), (1, 1)), 
        (Ipv4Addr::new(10, 0, 0, 2), (2, 1)), 
        (Ipv4Addr::new(10, 0, 0, 3), (0, 0)), 
        (Ipv4Addr::new(10, 0, 0, 4), (3, 1))
        ].into_iter().collect());

    assert_eq!(network.get_routing_table("r4".into()).await, [
        (Ipv4Addr::new(10, 0, 0, 1), (1, 2)), 
        (Ipv4Addr::new(10, 0, 0, 2), (1, 2)), 
        (Ipv4Addr::new(10, 0, 0, 3), (1, 1)), 
        (Ipv4Addr::new(10, 0, 0, 4), (0, 0))
        ].into_iter().collect());

    network.quit().await;

    Ok(())
}
