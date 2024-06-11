use std::sync::Arc;

use log::info;
use tokio::sync::{mpsc::{channel, Receiver, Sender}, Mutex};

#[derive(PartialEq, Eq, Clone)]
pub enum Source{
    OSPF,
    SPT,
    Ping,
    Debug
}

#[derive(Debug)]
pub struct Logger{
    sender: Arc<Mutex<Sender<(Source, String)>>>,
}

impl Logger{
    pub fn start() -> Logger{
        let (tx, rx) = channel(1024);
        tokio::spawn(async move{
            Self::write_loop(rx, vec![]).await
        });
        Logger{sender: Arc::new(Mutex::new(tx))}
    }

    pub fn start_with_filters(filters: Vec<Source>) -> Logger{
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