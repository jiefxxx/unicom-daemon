use std::collections::HashMap;
use super::LoggerMessage;

#[derive(Debug)]
pub struct Logs{
    logs : HashMap<String, Vec<String>>
}

impl Logs{
    pub fn new() -> Logs{
        Logs{
            logs: HashMap::new()
        }
    }

    pub fn new_log(&mut self, log: LoggerMessage){
        let value = match log{
            LoggerMessage::App { app, value, err, time } => {
                let log = match err {
                    true => format!("[{}]{}|[ERROR]{}", time.to_rfc3339(), app, value),
                    false => format!("[{}]{}|{}", time.to_rfc3339(), app, value),
                };
                self.add_app_log(&app, &log);
                log
            },
            LoggerMessage::Unicom { context, value, time } => {
                format!("[{}]unicom|{} : {:?}", time.to_rfc3339(), context, value)
            },
            LoggerMessage::Http { code, path, duration, time, method } => {
                format!("[{}]http|[{}]{} {} {}", time.to_rfc3339(), code, method, path, duration)
            },
        };

        println!("{}", value);

    }

    pub fn get_log(&self, app: &str) -> Option<&Vec<String>>{
        self.logs.get(app)
    }

    fn add_app_log(&mut self, app: &str, log: &str){
        let logs = match self.logs.get_mut(app) {
            Some(logs) => logs,
            None => {
                self.logs.insert(app.to_owned(), Vec::new());
                self.logs.get_mut(app).unwrap()
            },
        };
        logs.push(log.to_owned());
        if logs.len() > 300{
            logs.pop();
        }
    }
}

