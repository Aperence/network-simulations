
pub mod network;

use self::network::Network;

fn main() {
    let mut network = Network::new();
    network.add_switch("s1".into(), 1);
    network.add_switch("s2".into(), 2);
    network.add_switch("s3".into(), 3);
    network.add_switch("s4".into(), 4);
    network.add_switch("s5".into(), 5);
    network.add_switch("s6".into(), 6);

    network.add_link("s1".into(), 1, "s3".into(), 1, 1);
    network.add_link("s1".into(), 2, "s6".into(), 1, 1);
    network.add_link("s1".into(), 3, "s2".into(), 1, 4);
    network.add_link("s6".into(), 2, "s3".into(), 2, 3);
    network.add_link("s3".into(), 3, "s4".into(), 1, 1);
    network.add_link("s4".into(), 2, "s2".into(), 2, 1);
    network.add_link("s4".into(), 3, "s5".into(), 1, 1);
    network.add_link("s5".into(), 2, "s2".into(), 3, 1);

    network.run();

    network.print_switch_states();
    network.print_dot();
}
