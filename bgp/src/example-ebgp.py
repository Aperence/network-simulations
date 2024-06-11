from src import BGPNetwork

if __name__ == "__main__":
    network = BGPNetwork(verbose=True)

    network.add_router("r1", 1, 1)
    network.add_router("r2", 2, 2)
    network.add_router("r3", 3, 3)
    network.add_router("r4", 4, 4)
    network.add_router("r5", 5, 5)
    network.add_router("r6", 6, 6)
    network.add_router("r7", 7, 7)
    network.add_router("r8", 8, 8)

    network.add_peer_link("r2", "r3")
    network.add_peer_link("r4", "r5")
    network.add_peer_link("r5", "r6")
    network.add_peer_link("r6", "r8")

    network.add_provider_customer(provider="r3", customer="r1")
    network.add_provider_customer(provider="r1", customer="r2")
    network.add_provider_customer(provider="r4", customer="r3")
    network.add_provider_customer(provider="r5", customer="r2")
    network.add_provider_customer(provider="r7", customer="r4")
    network.add_provider_customer(provider="r6", customer="r7")
    network.add_provider_customer(provider="r8", customer="r7")

    network.announce_prefix("r2")

    network.print_bgp_tables()
    network.plot_network()
