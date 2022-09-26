use std::collections::HashMap;

use inflector::Inflector;
use serde_json::{from_value, json};
use tera::{Tera, Context, Value, Function};
use tokio::{sync::Mutex, task, runtime::Handle};
use unicom_lib::{node::NodeConfig, error::UnicomError};

use crate::SERVER;


#[derive(Debug)]

pub struct Render{
    tera: Mutex<Tera>,
}

impl Render{
    pub fn new(base_template_dir: &str) -> Render{
        let mut tera = Tera::new(base_template_dir).unwrap();
        tera.autoescape_on(vec![]);
        tera.register_filter("multidigit", multi_digit);
        tera.register_filter("bytes", bytes);
        tera.register_filter("duration", duration);
        tera.register_filter("capitalize_first", capitalize_first);
        tera.register_function("get_node_tag", get_node_tag());

        Render{
            tera: Mutex::new(tera),
        }
    }

    pub async fn render(&self, template_name: &str, context: &Context) -> Result<String, UnicomError>{
        let tera = self.tera.lock().await;
        Ok(tera.render(template_name, context)?)
    }   

    pub async fn add(&self, config: &NodeConfig) -> Result< (), UnicomError>{
        let mut tera = self.tera.lock().await;
        let mut files = Vec::new();
        for template in &config.templates{
            files.push((template.file.clone(), Some(&template.path)));
        }
        tera.add_template_files(files)?;
        Ok(())
    }

}

fn capitalize_first(v: &Value, _h: &HashMap<String, Value>) -> Result<Value, tera::Error>{
    let data = v.as_str().unwrap_or_default().to_title_case();
    Ok(Value::from(data))
}

fn multi_digit(v: &Value, _h: &HashMap<String, Value>) -> Result<Value, tera::Error>{
    let data = format!("{:02}",v.as_i64().unwrap_or_default());
    Ok(Value::from(data))
}
fn bytes(v: &Value, _h:& HashMap<String, Value>) -> Result<Value, tera::Error>{
    let size_name = ["B", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
    let value = v.as_f64().unwrap_or_default();
    let pre = value.ln()/(1024.0_f64).ln();
    let mut i = pre.floor();
    if i == 0.0 {
        i = 1.0;
    }
    let p = (1024.0_f64).powf(i);
    let s = value/p;
    return Ok(Value::from(format!("{:.1} {}", s, size_name[i as usize])));
}

fn duration(v: &Value, _h:& HashMap<String, Value>) -> Result<Value, tera::Error>{
    let value = v.as_i64().unwrap_or_default();
    let pre = value/60000;
    let hours = pre/60;
    let minutes = pre%60;
    if hours > 0{
        return Ok(Value::from(format!("{:}h {}min", hours, minutes)));
    }
    else{
        return Ok(Value::from(format!("{}min", minutes)));
    } 
}

fn get_node_tag() -> impl Function {
    Box::new(move |args: &HashMap<String, Value>| -> tera::Result<Value> {
        match args.get("tag") {
            Some(val) => match from_value::<String>(val.clone()) {
                Ok(tag) =>  {
                    Ok(json!(task::block_in_place(move || -> Vec<(String, String)>{
                        Handle::current().block_on(async move {
                            SERVER.controller.get_node_tag(&tag).await
                        })
                    })))
                },
                Err(_) => Err("oops".into()),
            },
            None => Err("oops".into()),
        }
    })
}