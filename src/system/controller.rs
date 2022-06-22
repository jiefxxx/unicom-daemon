use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use serde_json::json;
use tokio::time::sleep;
use unicom_lib::{node::{NodeConnector, NodeConfig, api::{ApiMethod, MethodKind, Parameter, ValueKind}, endpoint::EndPointKind, message::{request::UnicomRequest, response::UnicomResponse, UnicomMessage}}, error::UnicomError};

use crate::{server::controller::Controller, LOGGER};

#[derive(Debug, Deserialize, Serialize)]
pub struct LoginInput{
    login: String,
    password: String
}

pub struct SystemConnector{
    pub controller: Arc<Controller>,
}

#[async_trait]
impl NodeConnector for SystemConnector{

    async fn init(&self) -> Result<NodeConfig, UnicomError>{
        let mut config = NodeConfig::new("system");

        config.add_api(0, "nodes", vec![ApiMethod::new(MethodKind::GET, vec![
            Parameter::new("tag", ValueKind::String, false)])]);
        config.add_api(1, "apps", vec![ApiMethod::new(MethodKind::GET, vec![])]);
        config.add_api(2, "app_reload", vec![ApiMethod::new(MethodKind::GET, vec![
            Parameter::new("name", ValueKind::String, true)])]);
        config.add_api(3, "app_stop", vec![ApiMethod::new(MethodKind::GET, vec![
            Parameter::new("name", ValueKind::String, true)])]);
        config.add_api(4, "authenticate", vec![ApiMethod::new(MethodKind::POST, vec![
            Parameter::new("session_id", ValueKind::SessionID, true),
            Parameter::new("input", ValueKind::Input, true)])]);
        config.add_api(5, "app_log", vec![ApiMethod::new(MethodKind::GET, vec![
            Parameter::new("name", ValueKind::String, true)])]);

        config.add_endpoint("/system/nodes", EndPointKind::rest("node"));
        config.add_endpoint("/system/apps", EndPointKind::rest("apps"));
        config.add_endpoint("/system/apps/start", EndPointKind::rest("app_reload"));
        config.add_endpoint("/system/apps/stop", EndPointKind::rest("app_stop"));
        config.add_endpoint("/system/apps/logs", EndPointKind::rest("app_log"));
        config.add_endpoint("/system/authenticate", EndPointKind::rest("authenticate"));

        Ok(config)
    }

    async fn request(&self, request: UnicomRequest) -> Result<UnicomResponse, UnicomError>{
        match request.id{
            0 => {
                match request.parameters.get("tag"){
                    Some(tag_value) => {
                        let tag = tag_value.as_str().unwrap_or("");
                        UnicomResponse::from_json(&json!(self.controller.get_node_tag(tag).await))
                    },
                    None => UnicomResponse::from_json(&json!(self.controller.get_node_name().await)),
                }
            },
            1 => UnicomResponse::from_json(&json!(self.controller.apps.status().await?)),
            2 => {
                let name = request.parameters.get("name").unwrap().as_str().unwrap_or("");
                UnicomResponse::from_json(&json!(self.controller.apps.reload(name).await?))
            },
            3 => {
                let name = request.parameters.get("name").unwrap().as_str().unwrap_or("");
                UnicomResponse::from_json(&json!(self.controller.apps.stop(name).await?))
            },
            4 =>{
                let session_id = request.parameters.get("session_id").unwrap().as_str().unwrap_or("");
                let input: LoginInput = serde_json::from_value(request.parameters.get("input").unwrap().clone())?;
                
                

                UnicomResponse::from_json(&json!(self.controller.sessions.authentication(session_id, &input.login, &input.password).await?))
            }
            5 =>{
                let name = request.parameters.get("name").unwrap().as_str().unwrap_or("");
                UnicomResponse::from_json(&json!(LOGGER.logs.lock().await.get_log(name)))
            }
            _ => Ok(UnicomResponse::empty())
        }
        
    }
    async fn response(&self, _request_id: u64, _response: UnicomResponse) -> Result<(), UnicomError>{
        Ok(())//todo!()
    }
    async fn error(&self, request_id: u64, error: UnicomError) -> Result<(), UnicomError>{
        println!("système node config error {}{:?}", request_id, error);
        Ok(())
    }
    async fn next(&self) -> Result<UnicomMessage, UnicomError>{
        loop {
            sleep(Duration::from_secs_f32(100.0)).await;
        }
    }
    async fn quit(&self) -> Result<(), UnicomError>{
        println!("système node quit");
        Ok(())
    }

}