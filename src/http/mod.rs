use std::sync::Arc;

use futures::{StreamExt, TryStreamExt};
use tokio::{fs::File, io};
use unicom_lib::{error::{UnicomError, UnicomErrorKind}, node::api::{ApiMethod, ValueKind}};
use uuid::Uuid;


use hyper::{http::request, Body, header::{CONTENT_LENGTH, CONTENT_TYPE}};
use serde_json::{Map, json, Value};

use self::{input_file::InputFile, session::Session};

pub mod router;
pub mod render;
pub mod input_file;
pub mod session;

pub fn parse_parameters(parts: &request::Parts) -> Result<Map<String,Value>, UnicomError>{
    let mut raw_parameters = Map::new();
    if let Some(query) = parts.uri.query(){
        for raw_param in  query.split(";"){
            let param: Vec<&str> = raw_param.split("=").collect();
            if param.len() == 2{
                let value;
                if let Ok(f) = param[1].parse::<i64>(){
                    value = json!(f);
                }
                else if let Ok(f) = param[1].parse::<f64>(){
                    value = json!(f);
                }
                else{
                    value = json!(param[1]);
                }
                raw_parameters.insert(param[0].to_string(), value);
            }
        }
    }
    Ok(raw_parameters)
}

pub async fn parse_body(parts: &request::Parts, body: Body) -> Result<Option<Value>, UnicomError>{
    match parts.headers.get(CONTENT_LENGTH){
        Some(content_type) => {
            let length = content_type.to_str().unwrap();
            let i: usize = length.parse().unwrap();
            if i == 0{
                return Ok(None)
            }
        },
        None => return Ok(None),
    };
    let is_json = match parts.headers.get(CONTENT_TYPE){
        Some(content_type) => {
            let content_type = String::from(content_type.to_str().unwrap());
            content_type.starts_with("application/json")
        },
        None => false,
    };
    if is_json{
        let entire_body = match body.try_fold(Vec::new(), |mut data, chunk| async move {
            data.extend_from_slice(&chunk);
            Ok(data)
            }).await{
                Ok(value) => value,
                Err(e) => return Err(UnicomError::new(UnicomErrorKind::InputInvalid, &format!("try fold body error {:?}", e)))

            };
        match serde_json::from_slice(&entire_body){
            Ok(value) => Ok(Some(value)),
            Err(e) => Err(UnicomError::new(UnicomErrorKind::InputInvalid, &format!("parse body to json error {:?}", e))),
        }
    }
    else{
        let path = body_to_tmp_file(&parts, body).await?;
        Ok(Some(json!(InputFile{
            path,
        })))
    }
}

pub async fn body_to_tmp_file(request: &request::Parts, body: Body) -> Result<String, UnicomError>{
    if let Some(_size) = request.headers.get("Content-Length"){
        let mut body_stream = to_tokio_async_read(body.map(|result| result.map_err(|_error| std::io::Error::new(std::io::ErrorKind::Other, "Error!")))
            .into_async_read());

        let file_name = format!{"/tmp/unicom_post_{:}",Uuid::new_v4().to_string()};

        let mut file = File::create(&file_name).await?;

        io::copy(&mut body_stream, &mut file).await?;

        return Ok(file_name)
    }
    return Err(UnicomError::new(UnicomErrorKind::Empty, "content length undefined"))
}

fn to_tokio_async_read(r: impl futures::io::AsyncRead) -> impl tokio::io::AsyncRead {
    tokio_util::compat::FuturesAsyncReadCompatExt::compat(r)
}

pub fn add_http(api: &ApiMethod, parameters: &mut Map<String, Value>, url: Vec<String>, session: &Arc<Session>, input: Option<Value>){
    let mut input_name = None;
    
    for parameter in &api.parameters{
        println!("{}", parameter.name);
        match parameter.kind {
            ValueKind::Url(index) => {
                if index < url.len() && url[index].len() > 0{
                    parameters.insert(parameter.name.clone(), json!(url[index]));
                }
            },
            ValueKind::Input => {
                input_name = Some(parameter.name.clone())
            },
            ValueKind::SessionID => {
                parameters.insert(parameter.name.clone(), json!(session.id));
            }
            ValueKind::User => {
                parameters.insert(parameter.name.clone(), json!(session.get_user()));
            }
            _ => (),
        }
    }

    if input_name.is_some(){
        if input.is_some(){
            parameters.insert(input_name.unwrap(), input.unwrap());
        }
        else{
            parameters.insert(input_name.unwrap(), Value::Null);
        }
        
    }   
    
}