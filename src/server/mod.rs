use std::{sync::Arc, convert::Infallible, net::SocketAddr, path::Path, time::Duration};

use futures::future::join_all;
use hyper::{service::{make_service_fn, service_fn}, Request, Body, Response, StatusCode, header::{HeaderValue, SET_COOKIE}};
use serde_json::{Map, Value};
use tera::Context;
use tokio::{net::UnixListener, time::{Instant, sleep}};
use unicom_lib::{error::UnicomError, config::Config, node::{endpoint::{EndPointKind, ApiConfig}, api::MethodKind, message::{response::UnicomResponse, UnicomMessage, request::UnicomRequest}, NodeConnector, Node}};


use crate::{http::{self, input_file::InputFile, session::Session, add_http}, unix::UnixConnector, system::controller::SystemConnector, LOGGER};

use self::controller::Controller;


pub mod controller;

pub struct Server{
    unix_stream_path: String,
    controller: Arc<Controller>,
    server_addr: SocketAddr,
}

impl Server{

    pub fn new(config: &Config) -> Server{
        Server{
            unix_stream_path: config.unix_stream_path.clone(),
            controller: Arc::new(Controller::new(config)),
            server_addr: config.server_addr.parse().unwrap(),
        }
    }

    pub async fn run(&self) {
        tokio::spawn(Server::new_node(Arc::new(SystemConnector{ controller: self.controller.clone() }), self.controller.clone()));
        tokio::spawn(Server::unix_server(self.unix_stream_path.clone(), self.controller.clone()));
        tokio::spawn(Server::http_server(self.server_addr, self.controller.clone()));
        sleep(Duration::from_secs_f32(10.0)).await;
        if let Err(e) = self.controller.apps.init().await{
            LOGGER.error("apps init error", e).await;
        }
    }

    pub async fn stop(&self){
        self.controller.stop().await
    }

    async fn http_server(server_addr: SocketAddr, controller: Arc<Controller>){
        if let Err(e) = controller.sessions.load().await{
            LOGGER.error("error load session", e).await;
        }

        let make_service = make_service_fn(move |_conn| {
            let controller = controller.clone();
            async move {
                Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                    let controller = controller.clone();
                    async move {
                        Ok::<_, Infallible>(Server::http_worker(controller, req).await)
                    }
                }))
            }
        });
    
        hyper::Server::bind(&server_addr).serve(make_service).await.unwrap();
    }

    async fn http_worker(controller: Arc<Controller>, request: Request<Body>) -> Response<Body>{
        let mut cookie: Option<String> = None;
        let session = match controller.sessions.parse_session(&request).await{
            Some(session) => session,
            None => {
                let session = controller.sessions.create().await;
                cookie = Some(session.gen_cookies());
                session
            },
        }; 

        let path = request.uri().path().to_string();
        let method = request.method().clone();
        let start = Instant::now();
        let mut response = match Server::http_request(controller, request, session).await{
            Ok(response) => response,
            Err(e) => e.into(),
        };
        let duration = start.elapsed();

        let code = response.status();

        LOGGER.http(&path, code, &method, duration).await;

        if let Some(cookie) = cookie{
            response.headers_mut().append(SET_COOKIE, HeaderValue::from_str(&cookie).unwrap());
        }

        response
    }

    async fn http_request(controller: Arc<Controller>, request: Request<Body>, session: Arc<Session>) -> Result<Response<Body>, UnicomError>{
        let (parts, body) = request.into_parts();
        let (endpoint,node_name, url_var) = controller.router.find(parts.uri.path()).await?;
        match endpoint {
            EndPointKind::Static { path } => {
                match url_var.len(){
                    2 => Ok(hyper_staticfile::ResponseBuilder::new()
                        .request_parts(&parts.method,&parts.uri,&parts.headers)
                        .build(hyper_staticfile::resolve_path(&Path::new(&path), &url_var[1]).await.unwrap())
                        .unwrap()),

                    _ => Ok(hyper_staticfile::ResponseBuilder::new()
                        .request_parts(&parts.method,&parts.uri,&parts.headers)
                        .build(hyper_staticfile::resolve_path(&Path::new("/"), &path).await.unwrap())
                        .unwrap()),
                }
               
            },

            EndPointKind::Dynamic { api } => {
                let method: MethodKind = parts.method.clone().into();
                let mut param = http::parse_parameters(&parts)?;
                let node = controller.node(&node_name).await?;
                let api = node.api(&api)?;
                add_http(api.get_method(&method)?, &mut param, url_var, &session, http::parse_body(&parts, body).await?);
                let resp = node.request(api, method, param).await?;
                let file: InputFile = serde_json::from_str(&String::from_utf8(resp.data)?)?;
                Ok(hyper_staticfile::ResponseBuilder::new()
                    .request_parts(&parts.method,&parts.uri,&parts.headers)
                    .build(hyper_staticfile::resolve_path(&Path::new("/"), &file.path).await.unwrap())
                    .unwrap())
            },

            EndPointKind::Rest { api } => {
                let method: MethodKind = parts.method.clone().into();
                let mut param = http::parse_parameters(&parts)?;
                let node = controller.node(&node_name).await?;
                let api = node.api(&api)?;
                add_http(api.get_method(&method)?, &mut param, url_var, &session, http::parse_body(&parts, body).await?);
                let node_resp = node.request(api, method, param).await?;
                let string = String::from_utf8(node_resp.data)?;
                let mut resp = Response::builder().status(StatusCode::OK).body(Body::from(string)).unwrap();
                resp.headers_mut().insert("Content-Type", HeaderValue::from_str("application/json").unwrap());
                Ok(resp)
            },

            EndPointKind::View { apis, template } => {
                let method: MethodKind = parts.method.clone().into();
                let param = http::parse_parameters(&parts)?;
                let mut context = Context::new();
                let mut futures = Vec::new();
                let parsed_body = Box::new(http::parse_body(&parts, body).await?);

                for (key, config) in &apis{

                    let c_method = match &config.method{
                        Some(method) => method.clone(),
                        None => method.clone(),
                    };

                    let mut c_param = param.clone();
                    if let Some(d_param) = config.parameters.clone(){
                        c_param.extend(d_param);
                    }

                    futures.push(Server::execute_node(key, controller.clone(), config, c_method, c_param, &url_var, &session, parsed_body.clone()));
                }
    
                for result in join_all(futures).await{
                    let (key, resp) = result?;
                    let v: Value = serde_json::from_str(&String::from_utf8(resp.data)?)?;
                    context.insert(key, &v);
                }
                context.insert("source_node", &node_name);
                context.insert("user", &session.get_user());
    
                let output = controller.render.render(&template, &context).await?;
                
                let mut resp = Response::builder().status(StatusCode::OK).body(Body::from(output)).unwrap();
                resp.headers_mut().insert("Content-Type", HeaderValue::from_str("text/html; charset=utf-8").unwrap());
                Ok(resp)
            },
        }
    }

    async fn execute_node(key: &str, controller: Arc<Controller>, config: &ApiConfig, method: MethodKind,  
                            mut param: Map<String, Value>, url_var: &Vec<String>, session: &Arc<Session>,
                            parsed_body: Box<Option<Value>>) -> Result<(String, UnicomResponse), UnicomError>{
        let node = controller.node(&config.node).await?;
        let api = node.api(&config.api)?;
        
        add_http(api.get_method(&method)?, &mut param, url_var.clone(), session, *parsed_body);

        Ok((key.to_string(), node.request(api, method, param).await?))
    }


    async fn unix_server(stream_path: String, controller: Arc<Controller>){
        let listener = UnixListener::bind(stream_path).unwrap();
        loop{
            if let Ok((stream, _addr)) = listener.accept().await {
                tokio::spawn(Server::new_node(Arc::new(UnixConnector::new(stream)), controller.clone()));
            }

        }
    }

    async fn new_node(connector: Arc<dyn NodeConnector>, controller: Arc<Controller>){
        let node = match controller.new_node(connector.clone()).await{
            Ok(node) => node,
            Err(e) => {
                LOGGER.error("config error", e.clone()).await;
                connector.error(0, e).await.unwrap_or_default();
                return
            },
        };

        tokio::spawn(Server::working_node(node, controller.clone()));
    }

    async fn working_node(node: Arc<Node>, controller: Arc<Controller>){
        loop{
            match match node.next().await{
                Ok(message) => message,
                Err(e) => {
                    LOGGER.error("message error", e.clone()).await;
                    node.error(0, e).await.unwrap_or_default();
                    break;
                },
            } {
                UnicomMessage::Request { id, data } => {
                    tokio::spawn(Server::transaction_node(node.clone(), controller.clone(), id, data));
                },
                UnicomMessage::Quit => break,
            }
        }
        if let Err(e) = node.quit().await{
            LOGGER.error("message error on quit", e).await;
        }

        if let Err(e) = controller.remove_node(&node.name).await{
            LOGGER.error("remove node error", e).await;
        }
    }

    async fn transaction_node(node: Arc<Node>, controller: Arc<Controller>, request_id: u64, request: UnicomRequest){
        println!("new transaction {:?}", request);
        let target_node = match controller.node(&request.node_name).await{
            Ok(target_node) => target_node,
            Err(e) => {
                println!("get node erreur {:?}",e);
                node.error(request_id, e).await.unwrap_or_default();
                return
            },
        };

        let api = match target_node.api(&request.name){
            Ok(api) => api,
            Err(e) => {
                println!("get api erreur {:?}",e);
                node.error(request_id, e).await.unwrap_or_default();
                return
            },
        };

        match target_node.request(api, request.method, request.parameters).await{
            Ok(response) => {
                if let Err(e) = node.response(request_id, response.data).await{
                    println!("send node response erreur {:?}",e);
                    return
                }
            },

            Err(e) => {
                println!("request node erreur {:?}",e);
                node.error(request_id, e).await.unwrap_or_default();
                return
            },
        };
    }
}