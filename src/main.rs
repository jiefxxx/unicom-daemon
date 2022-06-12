use std::fs;

use tokio::signal;

use crate::log::Logger;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate lazy_static;

extern crate serde_json;

mod config;
mod unix;
mod system;
mod server;
mod http;
mod app;
mod log;

lazy_static! {
    static ref LOGGER: Logger = Logger::new();
}

#[tokio::main]
async fn main(){
    let config = config::read_config();
    if let Ok(_) = fs::remove_file(&config.unix_stream_path){
        println!("remove stream ");
    }
    let server = server::Server::new(&config);
    server.run().await;

    signal::ctrl_c().await.unwrap();

    server.stop().await;
}