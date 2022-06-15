use std::sync::Arc;

use tokio::sync::Mutex;
use unicom_lib::{node::{Node, NodeConnector}, config::Config, error::{UnicomError, UnicomErrorKind}};

use crate::{http::{router::Router, render::Render, session::SessionManager}, app::AppControler};

pub struct Controller{
    nodes:  Mutex<Vec<Arc<Node>>>,
    pub router: Router,
    pub render: Render,
    pub apps: AppControler,
    pub sessions: SessionManager,
}

impl Controller{
    pub fn new(config: &Config) -> Controller{
        Controller { 
            nodes: Mutex::new(Vec::new()),
            router: Router::new(),
            render: Render::new(&config.template_dir),
            apps: AppControler::new(&config.app_dir),
            sessions: SessionManager::new(&config.session_path),
        }
    }

    pub async fn stop(&self){
        let nodes = &mut *self.nodes.lock().await;
        loop{
            let node = match nodes.pop() {
                Some(node) => node,
                None => break,
            };
            if let Err(e) = node.quit().await{
                println!("quit error {}:{:?}", node.name, e);
            };
        }
        self.apps.close().await
    }

    pub async fn new_node(&self, connector:  Arc<dyn NodeConnector>)-> Result<Arc<Node>, UnicomError>{
        let mut nodes = self.nodes.lock().await;
        let config = connector.init().await?;
        println!("new node : {:?}", &config);
        let node = Node::new(&config, connector).await?;
        
        nodes.push(Arc::new(node));

        self.router.add(&config).await?;
        self.render.add(&config).await?;

        let node = nodes.last().unwrap().clone();
        self.apps.add_node(&node).await;
              
        Ok(node)   
    }

    pub async fn node(&self, name: &str) -> Result<Arc<Node>, UnicomError>{
        for node in &*self.nodes.lock().await{
            if node.name == name{
                return Ok(node.clone());
            }
        }
        Err(UnicomError::new(UnicomErrorKind::NotFound, &format!("node NOT FOUND")))
    }

    pub async fn get_node_name(&self) -> Vec<String>{
        let mut ret = Vec::new();
        for node in &*self.nodes.lock().await{
            ret.push(node.name.clone());
        }
        ret
    }

    pub async fn get_node_tag(&self, tag: &str) -> Vec<(String, String)>{
        let mut ret = Vec::new();
        for node in &*self.nodes.lock().await{
            match node.get_tag(tag).await{
                Some(tag_value) => ret.push((node.name.clone(), tag_value.clone())),
                None => continue,
            }
        }
        ret
    }

    pub async fn remove_node(&self, name: &str) -> Result<(), UnicomError>{
        let nodes = &mut *self.nodes.lock().await;
        for (index,node) in nodes.iter().enumerate(){
            if node.name == name{
                self.router.remove(node).await?;
                self.apps.remove_node(node).await;
                nodes.remove(index);
                break
            }
        }
        Err(UnicomError::new(UnicomErrorKind::NotFound, &format!("node NOT FOUND")))
    }
}
