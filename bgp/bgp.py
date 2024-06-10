import networkx as nx
from enum import Enum
from typing import Tuple
import matplotlib.pyplot as plt
from dataclasses import dataclass

@dataclass(unsafe_hash=True)
class BGPRoute:
    prefix: str
    nexthop: str
    as_path: Tuple[str]
    pref: int 
    med: int 
    router_id: int
    src: str = "ebgp"

    def __str__(self):
        return F"(Prefix={self.prefix}, nexthop={self.nexthop}, AS Path={self.as_path}, MED={self.med})"
    
    def __lt__(self, other):
        if (self.pref != other.pref):
            return self.pref > other.pref
        if (len(self.as_path) != len(other.as_path)):
            return len(self.as_path) < len(other.as_path)

class BGPMessage(Enum):
    UPDATE = 1
    WITHDRAW = 2

class BGPNetwork:
    def __init__(self, verbose=False, interactive=False) -> None:
        self.routers = {}
        self.AS = {}
        self.network = nx.DiGraph()
        self.bgp_tables = {}
        self.verbose = verbose or interactive
        self.interactive = interactive
        
    def add_router(self, name : str, AS : int, id : int):
        self.AS[AS] = self.AS.get(AS, nx.DiGraph())
        self.AS[AS].add_node(F"10.0.{AS}.{id}", name=name)
        self.network.add_node(name, AS=AS, id=id)
        self.bgp_tables[name] = {}
        
    def add_peer_link(self, r1 : str, r2 : str, med=0):
        self.network.add_edge(r1, r2, type="peer", med=med)
        self.network.add_edge(r2, r1, type="peer", med=med)
        
    def add_provider_customer(self, provider : str, customer : str, med=0):
        self.network.add_edge(provider, customer, type="customer", med=med)
        self.network.add_edge(customer, provider, type="provider", med=med)

    def add_internal_link(self, r1 : str, r2 : str, cost=1):
        self.network.add_edge(r1, r2, type="internal", cost=cost)
        self.network.add_edge(r2, r1, type="internal", cost=cost)
        data1 = self.network.nodes(data=True)[r1]
        data2 = self.network.nodes(data=True)[r2]
        assert data1["AS"] == data2["AS"]
        n = self.AS[data1["AS"]]
        prefix1 = F"10.0.{data1['AS']}.{data1['id']}"
        prefix2 = F"10.0.{data1['AS']}.{data2['id']}"
        n.add_edge(prefix1, prefix2, weight=cost)
        n.add_edge(prefix2, prefix1, weight=cost)


    def distance(self, router: str, nexthop: str):
        data = self.network.nodes(data=True)[router]
        path = nx.dijkstra_path(self.AS[data['AS']], F"10.0.{data['AS']}.{data['id']}", nexthop, weight='weight')
        return len(path)
        
    def decision_process(self, router : str, prefix : str):
        routes = self.bgp_tables[router].get(prefix, [])
        best = None
        # first pass to get minimum as-path/highest local pref
        for route in routes:
            if (best == None or route < best):
                best = route

        if (best == None):
            return None
        lowest_med = {}
        lowest_med[best.as_path[0]] = [best]
        for route in routes:
            if (route.pref != best.pref or len(route.as_path) != len(best.as_path)):
                continue
            
            if (route.as_path[0] in lowest_med):
                if (lowest_med[route.as_path[0]][0].med > route.med):
                    lowest_med[route.as_path[0]] = [route]
                elif lowest_med[route.as_path[0]][0].med == route.med:
                    lowest_med[route.as_path[0]].append(route)
            else:
                lowest_med[route.as_path[0]] = [route]

        routes = []
        for r in lowest_med.values():
            routes = routes + r

        best = None
        for route in routes:
            if (best == None):
                best = route
            elif (route.src != best.src):
                best = best if best.src == "ebgp" else route
            elif (route.src == "ibgp" and self.distance(router, route.nexthop) != self.distance(router, best.nexthop)):
                best = best if self.distance(router, best.nexthop) < self.distance(router, route.nexthop) else route
            else:
                best = best if best.router_id < route.router_id else route
        
        return best
    
    def update(self, route : BGPRoute, current : str, origin : str):
        AS = self.network.nodes(data=True)[current]["AS"]
        router_id_origin = self.network.nodes(data=True)[origin]["id"]
        current_id = self.network.nodes(data=True)[current]["id"]
        type_rel = "internal" if route.src == "ibgp" else self.network.adj[current][origin]["type"]

        previous_best = self.decision_process(current, route.prefix)
        routes = self.bgp_tables[current].get(route.prefix, set())
        pref = {
            "provider" : 50,
            "peer" : 100,
            "customer" : 150,
            "internal": route.pref
        }[type_rel]
        routes.add(BGPRoute(route.prefix, route.nexthop, route.as_path, pref, route.med, router_id_origin, src=route.src))
        self.bgp_tables[current][route.prefix] = routes
        best = self.decision_process(current, route.prefix)
        if (previous_best != best):
            # announce route to other relations
            for (_ip, router) in self.AS[AS].nodes(data=True):
                router = router["name"]
                if (router == current):
                    continue
                if (previous_best != None and previous_best.src != "ibgp"):
                    to_remove = BGPRoute(previous_best.prefix, F"10.0.{AS}.{current_id}", previous_best.as_path, previous_best.pref, previous_best.med, current_id, src="ibgp")
                    self.announce_route(to_remove, router, current, BGPMessage.WITHDRAW)
                new_route = BGPRoute(route.prefix, F"10.0.{AS}.{current_id}", route.as_path, pref, route.med, current_id, src="ibgp")
                if (route.src != "ibgp"):
                    self.announce_route(new_route, router, current, BGPMessage.UPDATE)
            new_route = BGPRoute(route.prefix, F"10.0.{AS}.{current_id}", (AS,) + route.as_path, -1, -1, -1)
            self.log(F"Router {current} has a new best route {new_route} to reach {route.prefix}")
            for (neigh, data) in self.network.adj[current].items():
                if (data["type"] != "internal"):
                    med = data["med"]
                    new_route.med = med
                    if (previous_best != None):
                        to_remove = BGPRoute(previous_best.prefix, F"10.0.{AS}.{current_id}", (AS,)+previous_best.as_path, -1, med, -1)
                        self.log(F"Router {current} withdraw route {to_remove} because new best route found")
                        self.announce_route(to_remove, neigh, current, BGPMessage.WITHDRAW)
                    if (type_rel != "customer" and self.network.adj[current][neigh]["type"] != "customer"):
                        continue # announcer routes learned from provider/peer only to customers
                    self.announce_route(new_route, neigh, current, BGPMessage.UPDATE)

    def withdraw(self, route : BGPRoute, current : str, origin : str):
        AS = self.network.nodes(data=True)[current]["AS"]
        current_id = self.network.nodes(data=True)[current]["id"]
        type_rel = "internal" if route.src == "ibgp" else self.network.adj[current][origin]["type"]
        best = self.decision_process(current, route.prefix)
        pref = {
            "provider" : 50,
            "peer" : 100,
            "customer" : 150,
            "internal": route.pref
        }[type_rel]
        route.pref = pref
        if (not route.prefix in self.bgp_tables[current]):
            return
        if (not route in self.bgp_tables[current][route.prefix]):
            return
        self.bgp_tables[current][route.prefix].remove(route)
        if (best == route):
            new_best = self.decision_process(current, route.prefix)
            if (new_best == None):
                return
            to_remove = BGPRoute(best.prefix, F"10.0.{AS}.{current_id}", (AS,) + best.as_path, -1, -1, -1)
            self.log(F"Router {current} lost its best route, withdrawing {to_remove}")
            for (_ip, router) in self.AS[AS].nodes(data=True):
                router = router["name"]
                if (best.src != "ibgp"):
                    to_remove = BGPRoute(best.prefix, F"10.0.{AS}.{current_id}", best.as_path, best.pref, best.med, current_id, src="ibgp")
                    self.announce_route(to_remove, router, current, BGPMessage.WITHDRAW)
                if (new_best.src != "ibgp"):
                    # if not a route learned from ibgp, announce it to other ibgp peers
                    to_announce = BGPRoute(new_best.prefix, F"10.0.{AS}.{current_id}", new_best.as_path, new_best.pref, new_best.med, current_id, "ibgp")
                    self.announce_route(to_announce, router, current, BGPMessage.UPDATE)
            for (neigh, data) in self.network.adj[current].items():
                if (data["type"] != "internal"):
                    med = data["med"]
                    to_remove.med = med
                    self.announce_route(to_remove, neigh, current, BGPMessage.WITHDRAW)
                    if (new_best.pref != 150 and data["type"] != "customer"):
                        continue # announce routes learned from provider/peer only to customers
                    if (new_best != None):
                        to_announce = BGPRoute(new_best.prefix, F"10.0.{AS}.{current_id}", (AS,) + new_best.as_path, -1, med, -1)
                        self.log(F"Router {current} announces new best route {to_announce}")
                        self.announce_route(to_announce, neigh, current, BGPMessage.UPDATE)

    def announce_route(self, route : BGPRoute, current : str, origin : str, type : BGPMessage):
        self.log(F"Router {current} received {type} from {origin} with route {route}")
        AS = self.network.nodes(data=True)[current]["AS"]
        if (AS in route.as_path):
            return # loop
        if (type == BGPMessage.UPDATE):
            self.update(route, current, origin)
        else:
            self.withdraw(route, current, origin)
    
    def str_ASPath(self, path : Tuple[int]):
        return ':'.join([F"AS{i}" for i in path])
        
    def announce_prefix(self, router : str):
        data = self.network.nodes(data=True)[router]
        prefix = F"10.0.{data['AS']}.0"
        route = BGPRoute(prefix, F"10.0.{data['AS']}.{data['id']}", (data['AS'],), 1000, 0, -1)
        self.bgp_tables[router][prefix] = {route}
        for neigh in self.network.adj[router]:
            route.med = self.network.adj[router][neigh]["med"]
            self.announce_route(route, neigh, router, BGPMessage.UPDATE)
            
    def print_bgp_tables(self):
        for AS in self.AS:
            print(F"AS {AS}:")
            for (_ip, router) in self.AS[AS].nodes(data=True):
                router = router["name"]
                print(F"  {router} :")
                for prefix in self.bgp_tables[router]:
                    best_route = self.decision_process(router, prefix)
                    print(F"    {prefix} :")
                    for route in self.bgp_tables[router][prefix]:
                        print(" "*5, end="")
                        print("*" if route == best_route else " ", end="")
                        print("i " if route.src == "ibgp" else "  ", end="")
                        as_path = self.str_ASPath(route.as_path)
                        print(F"nexthop={route.nexthop}, pref={route.pref:3d}, AS path={as_path}, MED={route.med}")
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
        internals = [(u,v) for u,v,e in self.network.edges(data=True) if e['type'] == 'internal']
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
        nx.draw_networkx_edges(
            self.network,
            pos,
            edgelist=internals,
            width=2,
            alpha=0.9,
            edge_color="black",
        )
        nx.draw_networkx_edge_labels(self.network, pos, {p : "$" for p in providers})
        nx.draw_networkx_edge_labels(self.network, pos, {p : "=" for p in peers})

        plt.show()
            
