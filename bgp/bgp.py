import networkx as nx
from enum import Enum
from typing import Tuple
import matplotlib.pyplot as plt

class BGPMessage(Enum):
    UPDATE = 1
    WITHDRAW = 2

class BGPNetwork:
    def __init__(self, verbose=False, interactive=False) -> None:
        self.network = nx.DiGraph()
        self.bgp_tables = {}
        self.verbose = verbose or interactive
        self.interactive = interactive
        
    def add_router(self, name : str, AS : int, id : int):
        self.network.add_node(name, AS=AS, id=id)
        self.bgp_tables[name] = {}
        
    def add_peer_link(self, r1 : str, r2 : str):
        self.network.add_edge(r1, r2, type="peer")
        self.network.add_edge(r2, r1, type="peer")
        
    def add_provider_customer(self, provider : str, customer : str):
        self.network.add_edge(provider, customer, type="customer")
        self.network.add_edge(customer, provider, type="provider")
        
    def decision_process(self, router : str, prefix : str):
        routes = self.bgp_tables[router].get(prefix, [])
        best = None
        for route in routes:
            (_, nexthop, as_path, pref, router_id) = route
            if (best == None):
                best = route
            elif (best[3] < pref):
                best = route  # higher local pref
            elif (best[3] == pref and len(best[2]) > len(as_path)):
                best = route  # shorter as-path
            elif (best[3] == pref and len(best[2]) == len(as_path) and best[4] > router_id):
                best = route
        return best
        
    def announce_route(self, route : Tuple[str, str, Tuple[int]], current : str, origin : str, type : BGPMessage):
        self.log(F"Router {current} received {type} from {origin} with route {self.str_route(route)}")
        AS = self.network.nodes(data=True)[current]["AS"]
        router_id_origin = self.network.nodes(data=True)[origin]["id"]
        type_rel = self.network.adj[current][origin]["type"]
        if (AS in route[2]):
            return # loop
        if (type == BGPMessage.UPDATE):
            (prefix, _, as_path) = route
            previous_best = self.decision_process(current, prefix)
            routes = self.bgp_tables[current].get(prefix, set())
            pref = {
                "provider" : 50,
                "peer" : 100,
                "customer" : 150
            }[type_rel]
            routes.add(route + (pref, router_id_origin)) # consider the router ID to be the AS number for simplicity
            self.bgp_tables[current][prefix] = routes
            best = self.decision_process(current, prefix)
            if (previous_best != best):
                # announce route to other relations
                new_route = (prefix, F"10.0.0.{AS}", (AS,) + as_path)
                self.log(F"Router {current} has a new best route {self.str_route(new_route)} to reach {prefix}")
                for neigh in self.network.adj[current]:
                    if (previous_best != None):
                        to_remove = (previous_best[0], F"10.0.0.{AS}", (AS,) + previous_best[2])
                        self.log(F"Router {current} withdraw route {self.str_route(to_remove)} because new best route found")
                        self.announce_route(to_remove, neigh, current, BGPMessage.WITHDRAW)
                    if (type_rel != "customer" and self.network.adj[current][neigh]["type"] != "customer"):
                        continue # announcer routes learned from provider/peer only to customers
                    self.announce_route(new_route, neigh, current, BGPMessage.UPDATE)
        else:
            # withdraw
            (prefix, nexthop, as_path) = route
            best = self.decision_process(current, prefix)
            pref = {
                "provider" : 50,
                "peer" : 100,
                "customer" : 150
            }[type_rel]
            if (not prefix in self.bgp_tables[current]):
                return
            if (not (prefix, nexthop, as_path, pref, router_id_origin) in self.bgp_tables[current][prefix]):
                return
            self.bgp_tables[current][prefix].remove((prefix, nexthop, as_path, pref, router_id_origin))
            if (best == (prefix, nexthop, as_path, pref, router_id_origin)):
                new_best = self.decision_process(current, prefix)
                if (new_best == None):
                    return
                pref = new_best[3]
                to_remove = (best[0], F"10.0.0.{AS}", (AS,) + best[2])
                self.log(F"Router {current} lost its best route, withdrawing {self.str_route(to_remove)}")
                for neigh in self.network.adj[current]:
                    self.announce_route(to_remove, neigh, current, BGPMessage.WITHDRAW)
                    if (pref != 150 and self.network.adj[current][neigh]["type"] != "customer"):
                        continue # announce routes learned from provider/peer only to customers
                    if (new_best != None):
                        to_announce = (new_best[0], F"10.0.0.{AS}", (AS,) + new_best[2])
                        self.log(F"Router {current} announces new best route {self.str_route(to_announce)}")
                        self.announce_route(to_announce, neigh, current, BGPMessage.UPDATE)
                    
    def str_route(self, route : Tuple[str, str, Tuple[int]]):
        return F"(Prefix={route[0]}, nexthop={route[1]}, AS Path={self.str_ASPath(route[2])})"
    
    def str_ASPath(self, path : Tuple[int]):
        return ':'.join([F"AS{i}" for i in path])
        
    def announce_prefix(self, router : str):
        AS = self.network.nodes(data=True)[router]["AS"]
        route = (F"10.0.0.{AS}", F"10.0.0.{AS}", (AS,))  # prefix, nexthop, AS-path
        self.bgp_tables[router][route[0]] = [route + (1000, -1)]
        for neigh in self.network.adj[router]:
            self.announce_route(route, neigh, router, BGPMessage.UPDATE)
            
    def print_bgp_tables(self):
        for router in self.bgp_tables:
            print(F"{router} :")
            for prefix in self.bgp_tables[router]:
                best_route = self.decision_process(router, prefix)
                print(F"  {prefix} :")
                for route in self.bgp_tables[router][prefix]:
                    if route == best_route:
                        print(" "*3 + "*", end="")
                    else:
                        print(" "*4, end="")
                    (_, nexthop, as_path, pref, router_id) = route
                    as_path = self.str_ASPath(as_path)
                    print(F"nexthop={nexthop}, pref={pref:3d}, AS path={as_path}")
        print()

    def log(self, txt : str):
        if (self.verbose):
            print(txt)
            if not self.interactive:
                return
            inp = ""
            print("Enter command: c to continue, p to pring bgp tables")
            while (inp != "c"):
                inp = input()
                if (inp == "p"):
                    self.print_bgp_tables()

        
    def plot_network(self):
        pos = nx.spring_layout(self.network)
        labels = { x : x for x in self.network.nodes}
        options = {"edgecolors": "tab:gray", "node_size": 500, "alpha": 0.9}
                
        peers = [(u,v) for u,v,e in self.network.edges(data=True) if e['type'] == 'peer']
        providers = [(u,v) for u,v,e in self.network.edges(data=True) if e['type'] == 'provider']
        nx.draw_networkx_nodes(self.network, pos, node_color="tab:blue", **options)
        nx.draw_networkx_labels(self.network, pos, labels, font_size=15)
        nx.draw_networkx_edges(
            self.network,
            pos,
            edgelist=peers,
            width=2,
            alpha=0.9,
            edge_color="tab:blue",
            label="="
        )
        nx.draw_networkx_edges(
            self.network,
            pos,
            edgelist=providers,
            width=2,
            alpha=0.9,
            edge_color="tab:red",
            label="$"
        )
        nx.draw_networkx_edge_labels(self.network, pos, {p : "$" for p in providers})
        nx.draw_networkx_edge_labels(self.network, pos, {p : "=" for p in peers})

        plt.show()
            
        
    
if __name__ == "__main__":
    network = BGPNetwork(verbose=True, interactive=True)
    network.add_router("r1", 1, 1)
    network.add_router("r2", 2, 2)
    network.add_router("r3", 3, 3)
    network.add_router("r4", 4, 4)
    network.add_router("r5", 5, 5)
    
    network.add_peer_link("r2", "r5")
    network.add_peer_link("r3", "r5")
    network.add_provider_customer(provider="r5", customer="r4")
    network.add_provider_customer(provider="r4", customer="r3")
    network.add_provider_customer(provider="r2", customer="r3")
    network.add_provider_customer(provider="r3", customer="r1")
    network.add_provider_customer(provider="r1", customer="r2")
    
    network.announce_prefix("r4")
    network.print_bgp_tables()
    #network.plot_network()