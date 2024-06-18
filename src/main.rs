
pub mod network;

use std::{collections::HashMap, env, thread, time::Duration};

use network::logger::{Logger, Source};
use strum::IntoEnumIterator;

use self::network::Network;

use serde_yaml::{self, Value};

fn generate_routers(network: &mut Network, config: &Value){
    let routers = &config["network"]["routers"];

    if routers.is_null(){
        return;
    }

    for router in routers.as_sequence().expect("Invalid format, routers config should be a list"){
        let name = router["name"].as_str().expect("name should be an string");
        let id = &router["id"].as_u64().expect("id should be an integer");
        let router_as = &router["AS"].as_u64().expect("AS should be an integer");
        network.add_router(name, *id as u32, *router_as as u32);

        println!("Added router {} with id {} in AS {}", name, id, router_as);
    }
}

fn generate_switchs(network: &mut Network, config: &Value){
    let switches = &config["network"]["switches"];

    if switches.is_null(){
        return;
    }

    for switch in switches.as_sequence().expect("Invalid format, switches config should be a list"){
        let name = &switch["name"].as_str().expect("name should be an string");
        let id = &switch["id"].as_u64().expect("id should be an integer");
        network.add_switch(name, *id as u32);

        println!("Added switch {} with id {}", name, id);
    }
}

async fn generate_links(network: &mut Network, config: &Value){
    let links = &config["network"]["links"];

    if links.is_null(){
        return;
    }

    let mut highest_port = HashMap::new();

    let internal = &links["internal"];
    if ! internal.is_null(){
        for link in internal.as_sequence().expect("Internal links should be a list"){
            let l = link.as_sequence().expect("Error parsing the two routers/switches of the link");
            let r1 = l[0].as_str().expect("Router/Switch name in link should be a string");
            let r2 = l[1].as_str().expect("Router/Switch name in link should be a string");
            let port1 = highest_port.entry(r1).or_insert(1);
            let port1_saved = *port1;
            *port1 += 1;
            let port2 = highest_port.entry(r2).or_insert(1);
            let port2_saved = *port2;
            *port2 += 1;
            
            let cost = 
                l.get(2)
                .unwrap_or(&Value::Number(1.into()))
                .as_u64()
                .expect("Cost should be an int");
    
            println!("Link from {}:{} to {}:{} added with cost {}", r1, port1_saved, r2, port2_saved, cost);
            network.add_link(r1, port1_saved, r2, port2_saved, cost as u32).await;
        }
    }


    let bgp = &links["bgp"];
    if bgp.is_null(){
        return;
    }

    let provider_customers = &bgp["provider-customer"];
    if !provider_customers.is_null(){
        for link in provider_customers.as_sequence().expect("BGP links should be a list"){
            let provider = link["provider"].as_str().expect("Provider name in link should be a string");
            let customer = link["customer"].as_str().expect("Customer name in link should be a string");
            let port1 = highest_port.entry(provider).or_insert(1);
            let port1_saved = *port1;
            *port1 += 1;
            let port2 = highest_port.entry(customer).or_insert(1);
            let port2_saved = *port2;
            *port2 += 1;
            
            let med = 
                link.get("med")
                .unwrap_or(&Value::Number(1.into()))
                .as_u64()
                .expect("MED should be an int");
    
            println!("BGP link from provider {}:{} to customer {}:{} added with med {}", provider, port1_saved, customer, port2_saved, med);
            network.add_provider_customer_link(provider, port1_saved, customer, port2_saved, med as u32).await;
        }
    }

    let peers = &bgp["peer"];
    if !peers.is_null(){
        for link in peers.as_sequence().expect("BGP links should be a list"){
            let l = link.as_sequence().expect("Error parsing the two routers/switches of the link");
            let r1 = l[0].as_str().expect("Router/Switch name in link should be a string");
            let r2 = l[1].as_str().expect("Router/Switch name in link should be a string");
            let port1 = highest_port.entry(r1).or_insert(1);
            let port1_saved = *port1;
            *port1 += 1;
            let port2 = highest_port.entry(r2).or_insert(1);
            let port2_saved = *port2;
            *port2 += 1;
            
            let med = 
                l.get(2)
                .unwrap_or(&Value::Number(1.into()))
                .as_u64()
                .expect("MED should be an int");
    
            println!("Peer link from {}:{} to {}:{} added with med {}", r1, port1_saved, r2, port2_saved, med);
            network.add_peer_link(r1, port1_saved, r2, port2_saved, med as u32).await;
        }
    }

    let ibgp = &bgp["ibgp"];
    if !ibgp.is_null(){
        for link in ibgp.as_sequence().expect("BGP links should be a list"){
            let l = link.as_sequence().expect("Error parsing the two routers/switches of the ibgp session");
            let r1 = l[0].as_str().expect("Router/Switch name in ibgp should be a string");
            let r2 = l[1].as_str().expect("Router/Switch name in ibgp should be a string");
    
            println!("IBGP session added between {} and {}", r1, r2);
            network.add_ibgp_connection(r1, r2).await;
        }
    }
}

async fn actions_first_round(network: &mut Network, config: &Value){
    let actions = &config["network"]["actions"];
    if actions.is_null(){
        return;
    }
    let announces = &actions["announce_prefix"];
    if !announces.is_null(){
        for announce in announces.as_sequence().expect("Announce prefix should be a list"){
            if announce.is_u64(){
                let announce = announce.as_u64().unwrap();
                network.announce_prefix_as(announce as u32).await;
            }else if announce.is_string(){
                let announce = announce.as_str().unwrap();
                network.announce_prefix(announce).await;
            }
        }
    }
    let print_routing_tables = &actions["print_routing_tables"];
    if !print_routing_tables.is_null(){
        println!("Routing tables:");
        network.print_routing_tables().await;
        println!("");
    }
    let print_port_states = &actions["print_port_states"];
    if !print_port_states.is_null(){
        println!("Switch port states:");
        network.print_switch_states().await;
        println!("");
    }
}

async fn actions_second_round(network: &mut Network, config: &Value){
    let actions = &config["network"]["actions"];
    if actions.is_null(){
        return;
    }
    let print_bgp_tables = &actions["print_bgp_tables"];
    if !print_bgp_tables.is_null(){
        println!("BGP tables:");
        network.print_bgp_tables().await;
        println!("");
    }
    let pings = &actions["ping"];
    if !pings.is_null(){
        let pings = pings.as_sequence().expect("Pings should be a list");
        for ping in pings{
            let from = ping["from"].as_str().expect("From should be a router name");
            let to = ping["to"].as_str().expect("To should be an ip address");
            network.ping(from, to.parse().expect("Failed to parse IP address")).await;
        }
    }
    let print_dot_graph = &actions["print_dot_graph"];
    if !print_dot_graph.is_null(){
        println!("DOT graph:");
        network.print_dot().await;
        println!("");
    }
}

fn get_logger(config: &Value) -> Logger{

    let config = &config["network"]["config"];
    if config.is_null(){
        return Logger::start();
    }
    let logs = &config["log"];
    if logs.is_null(){
        return Logger::start();
    }
    env::set_var("RUST_LOG", "debug");
    let mut logs_sources = vec![];
    for source in logs.as_sequence().expect("Logs should be a list"){
        let source = source.as_str().expect("Source should be a string");
        let source = match source{
            "OSPF" => Source::OSPF,
            "SPT" => Source::SPT,
            "PING" => Source::PING,
            "DEBUG" => Source::DEBUG,
            "IP" => Source::IP,
            "BGP" => Source::BGP,
            "ARP" => Source::ARP,
            s => {
                let sources: Vec<String> = Source::iter().map(|s| s.to_string()).collect();
                panic!("Unknown source {}, supported sources are [{}]", s, sources.join(", "));
            }
        };
        logs_sources.push(source);
    }
    Logger::start_with_filters(logs_sources)
}


#[tokio::main]
async fn main() -> Result<(), ()> {
    
    let file = std::env::args().nth(1).expect("Filename for configuration required");
    let f = std::fs::File::open(file).expect("File doesn't exists");
    let config: Value = serde_yaml::from_reader(f).expect("Error in yaml file");

    let logger = get_logger(&config);
    let mut network = Network::new(logger);

    generate_routers(&mut network, &config);
    generate_switchs(&mut network, &config);
    generate_links(&mut network, &config).await;
    
    // wait for convergence of IGP
    thread::sleep(Duration::from_millis(1000));

    actions_first_round(&mut network, &config).await;

    // wait for convergence of BGP
    thread::sleep(Duration::from_millis(2000));
    
    actions_second_round(&mut network, &config).await;

    // wait for pings
    thread::sleep(Duration::from_millis(1000));

    network.quit().await;

    env::remove_var("RUST_LOG");
    Ok(())
}
