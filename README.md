# Simulator of networks

In this repository, you will find a network simulator, that can, given a configuration file that describes the topology of the network, give the expected state of the different devices.

## Features

Currently supported features are:
- Adding a router
- Adding a switch
- Adding a link between 2 devices (switch/routers)
- Adding a BGP peer/provider-customer link between two routers
- Adding an iBGP connection between two routers
- Announcing its prefix for an AS/router
- Ping between routers
- Showing information about the state of devices :
  - routing table
  - BGP table
  - Port state for SPT protocol
- Having a trace of the messages exchanged in the network
- Getting a Graphiz representation of the network


## Using the simulator

To use the simulator, you simply have to first compile the project, using `cargo build --release`. This will generate an executable in `./target/release` which is the simulator.

You can then run it by using `network-simulator config.yaml`, by feeding the simulator a config file containing the topology of the network. Examples of such configuration files can be found in [the example folder](./examples/). 

By default, the traces of logs of the simulator are given on stderr, while the outputs (routing tables, BGP tables, ...) are printed on stdout. To separate those two, you can use `network-simulator config.yaml > stdout.txt 2> logs.txt`.

## Format of configuration file

The format of a configuration file is given by the following grammar:

```
Network ::= 
    network
        routers:
            List[RouterDef]
        switches:
            List[SwitchDef]
        links:
            Links
        config:
            Config
        actions:
            Actions
    
RouterDef ::= 
    name: str
    id: uint
    AS: uint

SwitchDef ::= 
    name: str
    id: uint

Links ::=
    internal: 
      List[InternalLinkConf]
    bgp:
      BGPLinks

InternalLinkConf 
    ::= [device1 (str), device2 (str), cost (uint)] 
      | [device1 (str), device2 (str)] // cost of 1 by default

BGPLinks ::= 
    provider-customer:
        List[ProviderCustomerLinkConf]
    peer:
        List[PeerLinkConf]
    ibgp:
        List[IBGPConnectionConf]

ProviderCustomerLinkConf ::=         
    provider: str
    customer: str

PeerLinkConf ::= 
    [str, str]

IBGPConnectionConf ::=
    [str, str]

Config ::=
    log: List[LogSource]

LogSource 
    ::= "ARP"
      | "BGP"
      | "DEBUG"
      | "IP"
      | "OSPF"
      | "PING"
      | "SPT"

Actions ::=
    announce_prefix: List[ToAnnounce]
    ping: List[PingConf]
    print_bgp_tables: bool     // print the bgp tables
    print_routing_tables: bool // print the routing tables
    dot_graph_file: str        // save the representation of network in file

ToAnnounce 
    ::= str     // single router announce its prefix
      | uint    // AS announce its prefix

PingConf ::=
    from: str  // router that will generate the ping
    to: str    // IP address to ping
```

## Architecture of the simulator

The simulator uses Tokio, a library allowing to define tasks in Rust to work. Typically, each device of the network will be represented by a task, that can be run concurrently on different threads. This allows us to represent more realistic situations. For the communication between the different devices, we use message-passing, which closely reflects how real network operate.