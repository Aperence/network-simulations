use std::{fmt::Display, sync::Arc};

use log::info;
use strum_macros::EnumIter;
use tokio::sync::{mpsc::{channel, Receiver, Sender}, Mutex};

#[derive(EnumIter, PartialEq, Eq, Clone)]
pub enum Source{
    OSPF,
    SPT,
    PING,
    DEBUG,
    IP,
    BGP,
    ARP
}

impl Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self{
            Source::OSPF => "OSPF",
            Source::SPT => "SPT",
            Source::PING => "PING",
            Source::DEBUG => "DEBUG",
            Source::IP => "IP",
            Source::BGP => "BGP",
            Source::ARP => "ARP",
        };
        write!(f, "{}", str)
    }
}

#[derive(Debug)]
pub struct Logger{
    sender: Arc<Mutex<Sender<(Source, String)>>>,
}

impl Logger{
    pub fn start_test() -> Logger{
        let (tx, rx) = channel(1024);
        tokio::spawn(async move{
            Self::write_loop(rx, vec![]).await
        });
        Logger{sender: Arc::new(Mutex::new(tx))}
    }

    pub fn start() -> Logger{
        env_logger::init();
        let (tx, rx) = channel(1024);
        tokio::spawn(async move{
            Self::write_loop(rx, vec![]).await
        });
        Logger{sender: Arc::new(Mutex::new(tx))}
    }

    pub fn start_with_filters(filters: Vec<Source>) -> Logger{
        env_logger::init();
        let (tx, rx) = channel(1024);
        tokio::spawn(async move{
            Self::write_loop(rx, filters).await
        });
        Logger{sender: Arc::new(Mutex::new(tx))}
    }

    pub async fn write_loop(mut receiver: Receiver<(Source, String)>, filters: Vec<Source>){
        loop{
            match receiver.recv().await{
                Some((src, msg)) => {
                    if filters.len() > 0 && !filters.contains(&src){
                        continue;
                    }
                    info!("{}", msg);
                },
                None => break,
            }
        }
    }

    pub async fn log(&self, src: Source, msg: String){
        self.sender.lock().await.send((src, msg)).await.expect("Failed to log");
    }

    pub fn clone(&self) -> Logger{
        Logger{sender: Arc::clone(&self.sender)}
    }
}