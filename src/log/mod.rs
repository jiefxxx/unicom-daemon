use std::{time::Duration, sync::Arc};

use chrono::{Local, DateTime};
use hyper::{StatusCode, Method};
use tokio::sync::{mpsc::{Sender, self}, Mutex};
use unicom_lib::error::UnicomError;

use self::logs::Logs;

mod logs;

#[derive(Debug)]
pub enum LoggerMessage{
    App {
        app: String,
        value: String,
        err: bool,
        time: DateTime<Local>,
    },

    Unicom {
        context: String,
        value: UnicomError,
        time: DateTime<Local>,
    },

    Http {
        code: String,
        method: String,
        path: String,
        duration: f32,
        time: DateTime<Local>,
    }
}

#[derive(Debug)]
pub struct Logger{
    tx: Sender<LoggerMessage>,
    pub logs: Arc<Mutex<Logs>>,
}

impl Logger{
    pub fn new() -> Logger{
        let (tx, mut rx) = mpsc::channel(64);
        let logs = Arc::new(Mutex::new(Logs::new()));
        let ret = Logger { tx, logs: logs.clone()  };
        tokio::spawn(async move{
            loop{
                let log = rx.recv().await;
                if log.is_none(){
                    break
                }
                logs.lock().await.new_log(log.unwrap());
            }

        });
        ret
    }

    pub async fn app_stdout(&self, name: &str, value: String){
        self.tx.send(LoggerMessage::App { 
            app: name.to_owned(), 
            value, err: false, 
            time: Local::now() 
        }).await.expect("log tx send error");
    }

    pub async fn app_stderr(&self, name: &str, value: String){
        self.tx.send(LoggerMessage::App { 
            app: name.to_owned(), 
            value, err: true, 
            time: Local::now() 
        }).await.expect("log tx send error");
    }

    pub async fn error(&self, context: &str, value: UnicomError){
        self.tx.send(LoggerMessage::Unicom { 
            context: context.to_owned(), 
            value, 
            time: Local::now() 
        }).await.expect("log tx send error");
    }

    pub async fn http(&self, path: &str, code: StatusCode, method: &Method, duration: Duration){
        self.tx.send(LoggerMessage::Http { 
            code: code.to_string(), 
            method: method.to_string(),
            path: path.to_owned(), 
            duration: duration.as_secs_f32(), 
            time: Local::now()
        }).await.expect("log tx send error");
    }

}