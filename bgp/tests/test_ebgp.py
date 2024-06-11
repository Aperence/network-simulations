import unittest
from src.bgp import BGPNetwork, BGPRoute


class BGPTest(unittest.TestCase):

    def test(self):
        expected_routes = {
            "r1": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.3.3",
                        as_path=(3, 2),
                        pref=50,
                        med=0,
                        router_id=3,
                        src="ebgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.2.2",
                        as_path=(2,),
                        pref=150,
                        med=0,
                        router_id=2,
                        src="ebgp",
                    ),
                }
            },
            "r2": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.2.2",
                        as_path=(2,),
                        pref=1000,
                        med=0,
                        router_id=-1,
                        src="ebgp",
                    )
                }
            },
            "r3": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.2.2",
                        as_path=(2,),
                        pref=100,
                        med=0,
                        router_id=2,
                        src="ebgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.1.1",
                        as_path=(1, 2),
                        pref=150,
                        med=0,
                        router_id=1,
                        src="ebgp",
                    ),
                }
            },
            "r4": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.3.3",
                        as_path=(3, 1, 2),
                        pref=150,
                        med=0,
                        router_id=3,
                        src="ebgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.5.5",
                        as_path=(5, 2),
                        pref=100,
                        med=0,
                        router_id=5,
                        src="ebgp",
                    ),
                }
            },
            "r5": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.2.2",
                        as_path=(2,),
                        pref=150,
                        med=0,
                        router_id=2,
                        src="ebgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.4.4",
                        as_path=(4, 3, 1, 2),
                        pref=100,
                        med=0,
                        router_id=4,
                        src="ebgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.6.6",
                        as_path=(6, 7, 4, 3, 1, 2),
                        pref=100,
                        med=0,
                        router_id=6,
                        src="ebgp",
                    ),
                }
            },
            "r6": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.7.7",
                        as_path=(7, 4, 3, 1, 2),
                        pref=150,
                        med=0,
                        router_id=7,
                        src="ebgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.5.5",
                        as_path=(5, 2),
                        pref=100,
                        med=0,
                        router_id=5,
                        src="ebgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.8.8",
                        as_path=(8, 7, 4, 3, 1, 2),
                        pref=100,
                        med=0,
                        router_id=8,
                        src="ebgp",
                    ),
                }
            },
            "r7": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.4.4",
                        as_path=(4, 3, 1, 2),
                        pref=150,
                        med=0,
                        router_id=4,
                        src="ebgp",
                    )
                }
            },
            "r8": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.7.7",
                        as_path=(7, 4, 3, 1, 2),
                        pref=150,
                        med=0,
                        router_id=7,
                        src="ebgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.6.6",
                        as_path=(6, 7, 4, 3, 1, 2),
                        pref=100,
                        med=0,
                        router_id=6,
                        src="ebgp",
                    ),
                }
            },
        }

        # to tackle race conditions, try more than once
        for _ in range(20):
            network = BGPNetwork()

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

            self.assertEqual(expected_routes, network.bgp_tables)


if __name__ == "__main__":
    unittest.main()
