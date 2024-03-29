use std::{fs, sync::Arc};

use tokio::sync::Notify;

use crate::log::Logger;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate lazy_static;

extern crate serde_json;

mod unix;
mod system;
mod server;
mod http;
mod app;
mod log;

use unicom_lib::config::Config;

lazy_static! {
    static ref LOGGER: Logger = Logger::new();
}

lazy_static! {
    static ref SERVER: server::Server = {
        let config = read_config();
        if let Ok(_) = fs::remove_file(&config.unix_stream_path){
            println!("remove stream ");
        }
        server::Server::new(&config)
    };
}

#[tokio::main]
async fn main(){
    let close_notify = Arc::new(Notify::new());
    let close_notify_clone = close_notify.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        close_notify_clone.notify_one()
    });
            
    let config = read_config();
    if let Ok(_) = fs::remove_file(&config.unix_stream_path){
        println!("remove stream ");
    }

    SERVER.run().await;

    close_notify.notified().await;

    SERVER.stop().await;
}

pub fn read_config() -> Config{
    let content = if std::path::Path::new("./config.toml").exists(){
        std::fs::read_to_string("./config.toml").unwrap()
    }
    else{
        std::fs::read_to_string("/etc/unicom/config.toml").unwrap()
    };
    toml::from_str(&content).unwrap()
    
}