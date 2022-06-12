use std::{sync::Arc, fs, path::Path};

use tokio::sync::Mutex;
use unicom_lib::{error::UnicomError, node::Node};

use self::app::{App, AppConfig, AppState};

mod app;

pub struct AppControler{
    apps: Mutex<Vec<Arc<App>>>,
    location: String,
    

}

impl AppControler{
    pub fn new(location: &str) -> AppControler{
        AppControler{
            apps: Mutex::new(Vec::new()),
            location: location.to_string(),
        }
    }

    pub async fn init(&self) -> Result<(),UnicomError>{
        for path in fs::read_dir(Path::new(&self.location))?{
            let path = path?.path();
            if path.is_dir() {
                self.load(path.to_str().unwrap(), false).await?;
            }
        }

        self.start_app(None).await;
        
        Ok(())
    }

    pub async fn add_node(&self, node: &Arc<Node>){
        for app in &*self.apps.lock().await{
            if app.config.name == node.name{
                app.set_running().await;
                break
            }
        }
        self.start_app(Some(&node.name)).await;
    }

    pub async fn remove_node(&self, node: &Arc<Node>){
        for app in &*self.apps.lock().await{
            if app.config.name == node.name{
                app.set_zombie().await;
            }
        }
    }

    pub async fn status(&self) -> Result<Vec<(String, AppState)>, UnicomError>{
        let mut ret = Vec::new();
        for app in &*self.apps.lock().await{
            ret.push((app.config.name.clone(), app.get_state().await))
        }
        Ok(ret)
    }

    pub async fn reload(&self, name: &str) -> Result<(), UnicomError>{
        let app = self.app(name).await?;
        self.load(&app.dir, true).await?;
        Ok(())
    }

    pub async fn stop(&self, name: &str) -> Result<(), UnicomError>{
        self.app(name).await?.stop().await?;
        Ok(())
    }

    pub async fn close(&self){
        let apps = &mut *self.apps.lock().await;
        loop{
            let app = match apps.pop(){
                Some(app) => app,
                None => break,
            };

            if let Err(e) = app.stop().await{
                println!("stop app error {}:{:?}", app.config.name, e);
            };
        }
    }


    async fn load(&self, dir: &str, reload: bool) -> Result<(), UnicomError>{
        let config = AppConfig::read_config(dir).await?;

        if let Some(index) = self.get_app(&config.name).await{
            if !reload{
                return Err(UnicomError::new(unicom_lib::error::UnicomErrorKind::ParameterInvalid, &format!("app name already exist {}", &config.name)))
            }
            let app = self.apps.lock().await.remove(index);
            app.stop().await?;

        }

        let app = self.create_app(dir, config).await;

        if reload{
            app.start().await?;
        }

        Ok(())
    }

    async fn get_app(&self, name: &str) -> Option<usize>{
        let apps = self.apps.lock().await;
        for (index, app) in apps.iter().enumerate(){
            if app.config.name == name{
                return Some(index)
            }
        }
        None
    }

    async fn app(&self, name: &str) -> Result<Arc<App>, UnicomError>{
        match self.get_app(name).await{
            Some(index) => {
                Ok(self.apps.lock().await[index].clone())
            },
            None => return Err(UnicomError::new(unicom_lib::error::UnicomErrorKind::NotFound, 
                            &format!("app not found {}", name))),
        }
    }

    async fn create_app(&self, dir: &str, config: AppConfig) -> Arc<App>{
        let app = Arc::new(App::new(dir, config));
        let ret = app.clone();
        let mut apps = self.apps.lock().await;
        apps.push(app);

        return ret
    }

    async fn start_app(&self, after: Option<&String>){
        for app in &*self.apps.lock().await{
            if app.config.after.is_none(){
                if let Err(e) = app.start().await{
                    println!("Error will starting {} : {:?}", app.config.name, e);
                }
            } else if after.is_some() && app.config.after.as_ref().unwrap() == after.unwrap(){
                if let Err(e) = app.start().await{
                    println!("Error will starting {} : {:?}", app.config.name, e);
                }
            }
        }
    }
}
