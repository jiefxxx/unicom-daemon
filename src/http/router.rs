use std::sync::Arc;

use regex::Regex;
use tokio::sync::Mutex;
use unicom_lib::{node::{endpoint::EndPointKind, NodeConfig, Node}, error::{UnicomError, UnicomErrorKind}};

#[derive(Debug, Clone)]
pub struct Route{
    regex: Regex,
    kind: EndPointKind,
    node: String,
}

pub struct Router{
    routes: Mutex<Vec<Route>>,
}

impl Router{
    pub fn new() -> Router{
        Router{
            routes: Mutex::new(Vec::new()),
        }
    }

    pub async fn add(&self, config: &NodeConfig) -> Result<(), UnicomError>{
        let mut routes = self.routes.lock().await;
        for endpoint in &config.endpoints{
            routes.push(Route{
                regex: Regex::new(&format!("^{}$",endpoint.regex))?,
                kind: endpoint.kind.clone(),
                node: config.name.clone(),
            });
        }
        Ok(())
    }

    pub async fn find(&self, path: &str) -> Result<(EndPointKind, String, Vec<String>), UnicomError>{
        for route in &*self.routes.lock().await{
            for cap in route.regex.captures_iter(path) {
                let url = cap.iter().map(|value| {
                    match value {
                        Some(string) => string.as_str().to_string(),
                        None => String::new(),
                    }
                } ).collect();
                return Ok((route.kind.clone(), route.node.clone(), url))
            }
        }
        Err(UnicomError::new(UnicomErrorKind::NotFound, &format!("url {} not found", path)))
    }

    pub async fn remove(&self, node: &Arc<Node>) -> Result<(), UnicomError>{
        let mut routes = self.routes.lock().await;
        for index in (0..routes.len()).rev(){
            if node.name == routes[index].node{
                routes.swap_remove(index);
            }
        }
        Ok(())
    }
   
}
