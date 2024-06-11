use crate::network::PortState;
use crate::network::messages::Message;
use std::{cell::RefCell, collections::{BTreeMap, HashMap}, net::Ipv4Addr, rc::Rc};
use tokio::sync::mpsc::{Receiver, Sender};

pub enum Command{
    StatePorts,
    RoutingTable,
    AddLink(Receiver<Message>, Sender<Message>, u32, u32),
    Ping(Ipv4Addr),
    Quit
}

pub enum Response{
    StatePorts(BTreeMap<u32, PortState>),
    RoutingTable(HashMap<Ipv4Addr, (u32, u32)>)
}

#[derive(Debug)]
pub struct SwitchCommunicator{
    pub command_sender: Sender<Command>, 
    pub response_receiver: Rc<RefCell<Receiver<Response>>>
}

impl SwitchCommunicator {

    pub async fn add_link(&self, receiver: Receiver<Message>, sender: Sender<Message>, port: u32, cost: u32) {
        self.command_sender.send(Command::AddLink(receiver, sender, port, cost)).await.expect("Failed to send add link command");
    }

    pub async fn quit(self){
        self.command_sender.send(Command::Quit).await.expect("Failed to send quit message");
    }

    pub async fn get_port_state(&self) -> Result<BTreeMap<u32, PortState>, ()>{
        self.command_sender.send(Command::StatePorts).await.expect("Failed to send StatePorts message");
        match self.response_receiver.borrow_mut().recv().await{
            Some(Response::StatePorts(ports)) => Ok(ports),
            Some(Response::RoutingTable(_)) => panic!("Unexpected answer"),
            None => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct RouterCommunicator{
    pub command_sender: Sender<Command>, 
    pub response_receiver: Rc<RefCell<Receiver<Response>>>
}

impl RouterCommunicator {
    pub async fn add_link(&self, receiver: Receiver<Message>, sender: Sender<Message>, port: u32, cost: u32) {
        self.command_sender.send(Command::AddLink(receiver, sender, port, cost)).await.expect("Failed to send add link command");
    }

    pub async fn ping(&self, ip: Ipv4Addr){
        self.command_sender.send(Command::Ping(ip)).await.expect("Failed to send ping command");
    }

    pub async fn get_routing_table(&self) -> Result<HashMap<Ipv4Addr, (u32, u32)>, ()>{
        self.command_sender.send(Command::RoutingTable).await.expect("Failed to send RoutingTable message");
        match self.response_receiver.borrow_mut().recv().await{
            Some(Response::StatePorts(_)) => panic!("Unexpected answer"),
            Some(Response::RoutingTable(table)) => Ok(table),
            None => Err(()),
        }
    }

    pub async fn quit(self){
        self.command_sender.send(Command::Quit).await.expect("Failed to send quit command");
    }
}