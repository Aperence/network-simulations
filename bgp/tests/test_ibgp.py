import unittest
from src.bgp import BGPNetwork, BGPRoute


class BGPTest(unittest.TestCase):

    def test(self):
        expected_routes = {
            "r1": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.1.3",
                        as_path=(5, 2),
                        pref=50,
                        med=0,
                        router_id=3,
                        src="ibgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.1.4",
                        as_path=(4, 2),
                        pref=50,
                        med=0,
                        router_id=4,
                        src="ibgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.3.31",
                        as_path=(3, 5, 2),
                        pref=50,
                        med=7,
                        router_id=31,
                        src="ebgp",
                    ),
                }
            },
            "r2": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.1.3",
                        as_path=(5, 2),
                        pref=50,
                        med=0,
                        router_id=3,
                        src="ibgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.1.4",
                        as_path=(4, 2),
                        pref=50,
                        med=0,
                        router_id=4,
                        src="ibgp",
                    ),
                }
            },
            "r3": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.5.51",
                        as_path=(5, 2),
                        pref=50,
                        med=0,
                        router_id=51,
                        src="ebgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.1.4",
                        as_path=(4, 2),
                        pref=50,
                        med=0,
                        router_id=4,
                        src="ibgp",
                    ),
                }
            },
            "r4": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.1.3",
                        as_path=(5, 2),
                        pref=50,
                        med=0,
                        router_id=3,
                        src="ibgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.4.42",
                        as_path=(4, 2),
                        pref=50,
                        med=0,
                        router_id=42,
                        src="ebgp",
                    ),
                }
            },
            "r5": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.1.3",
                        as_path=(5, 2),
                        pref=50,
                        med=0,
                        router_id=3,
                        src="ibgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.4.41",
                        as_path=(4, 2),
                        pref=50,
                        med=3,
                        router_id=41,
                        src="ebgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.1.4",
                        as_path=(4, 2),
                        pref=50,
                        med=0,
                        router_id=4,
                        src="ibgp",
                    ),
                }
            },
            "r6": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.1.3",
                        as_path=(5, 2),
                        pref=50,
                        med=0,
                        router_id=3,
                        src="ibgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.3.31",
                        as_path=(3, 5, 2),
                        pref=50,
                        med=1,
                        router_id=31,
                        src="ebgp",
                    ),
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.1.4",
                        as_path=(4, 2),
                        pref=50,
                        med=0,
                        router_id=4,
                        src="ibgp",
                    ),
                }
            },
            "r21": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.2.21",
                        as_path=(2,),
                        pref=1000,
                        med=0,
                        router_id=-1,
                        src="ebgp",
                    )
                }
            },
            "r31": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.5.51",
                        as_path=(5, 2),
                        pref=50,
                        med=0,
                        router_id=51,
                        src="ebgp",
                    )
                }
            },
            "r41": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.2.21",
                        as_path=(2,),
                        pref=50,
                        med=0,
                        router_id=21,
                        src="ebgp",
                    )
                }
            },
            "r42": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.4.41",
                        as_path=(2,),
                        pref=50,
                        med=0,
                        router_id=41,
                        src="ibgp",
                    )
                }
            },
            "r51": {
                "10.0.2.0": {
                    BGPRoute(
                        prefix="10.0.2.0",
                        nexthop="10.0.2.21",
                        as_path=(2,),
                        pref=50,
                        med=0,
                        router_id=21,
                        src="ebgp",
                    )
                }
            },
        }

        # to tackle race conditions, try more than once
        for _ in range(20):
            network = BGPNetwork()

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

            self.assertEqual(expected_routes, network.bgp_tables)


if __name__ == "__main__":
    unittest.main()
