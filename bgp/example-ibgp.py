from bgp import BGPNetwork

if __name__ == "__main__":
    network = BGPNetwork(verbose=True)

    # AS1
    network.add_router("r1", 1, 1)
    network.add_router("r2", 1, 2)
    network.add_router("r3", 1, 3)
    network.add_router("r4", 1, 4)
    network.add_router("r5", 1, 5)
    network.add_router("r6", 1, 6)

    # AS2
    network.add_router("r21", 2, 21)

    # AS3
    network.add_router("r31", 3, 31)

    # AS4
    network.add_router("r41", 4, 41)
    network.add_router("r42", 4, 42)

    # AS5
    network.add_router("r51", 5, 51)
    
    network.add_provider_customer(provider="r21", customer="r51")
    network.add_provider_customer(provider="r21", customer="r41")
    network.add_provider_customer(provider="r41", customer="r5", med=3)
    network.add_provider_customer(provider="r42", customer="r4", med=0)
    network.add_provider_customer(provider="r51", customer="r3")
    network.add_provider_customer(provider="r31", customer="r1", med=7)
    network.add_provider_customer(provider="r31", customer="r6", med=1)
    network.add_provider_customer(provider="r51", customer="r31")

    network.add_internal_link("r1", "r6")
    network.add_internal_link("r3", "r6", cost=3)
    network.add_internal_link("r1", "r2")
    network.add_internal_link("r1", "r3")
    network.add_internal_link("r2", "r4")
    network.add_internal_link("r5", "r6")
    network.add_internal_link("r4", "r5", cost=7)

    network.add_internal_link("r41", "r42", cost=2)

    network.announce_prefix("r21")

    network.print_bgp_tables()
    network.plot_network()