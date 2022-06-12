use std::{path::Path, sync::Arc, process::{ExitStatus, Stdio}};

use nix::{sys::signal::{self, Signal}, unistd::Pid};
use tokio::{fs, task::JoinHandle, sync::Mutex, process::Command, io::{BufReader, AsyncBufReadExt}};
use unicom_lib::error::UnicomError;

use crate::LOGGER;

pub struct AppProcess{
    name: String,
    pid : u32,
    handle: JoinHandle<Result<ExitStatus, UnicomError>>,
}

impl AppProcess {
    pub fn new(cmd: &mut Command, name: String) -> Result<AppProcess, UnicomError>{
        // let mut child = cmd.spawn()?;
        let mut child = cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped()).spawn()?;

        let pid = child.id().unwrap();
        let stdout = child.stdout.take().expect("child did not have a handle to stdout");
        let stderr = child.stderr.take().expect("child did not have a handle to stderr");

        let name1 = name.clone();
        let name2 = name.clone();

        tokio::spawn(async move{
            
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                LOGGER.app_stdout(&name1, line).await;
            }
        });

        tokio::spawn(async move{
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                LOGGER.app_stderr(&name2, line).await;
            }
        });

        let handle= tokio::spawn( async move {
            let status =  child.wait().await?;
            Ok(status)
        });

        Ok(AppProcess{
            pid,
            handle,
            name
        })
    }

    pub async fn stop(&mut self) -> Result<(), UnicomError>{
        println!("{}", self.pid);
        signal::kill(Pid::from_raw(self.pid as i32), Signal::SIGINT).unwrap();
        let fut = &mut self.handle;
        let status = fut.await.unwrap()?;
        LOGGER.app_stdout(&self.name, format!("stoped with code {}", status)).await;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum AppState{
    Waiting,
    Started,
    Running,
    Zombie,
    Stoped,
}


pub struct App{
    pub config: AppConfig,
    pub dir: String,
    connection: Mutex<Option<AppProcess>>,
    pub auto_reload: Arc<Mutex<bool>>,
    pub state: Mutex<AppState>

}

impl App{
    pub fn new(dir: &str, config: AppConfig) -> App{
        let auto_reload = Arc::new(Mutex::new(config.auto_reload.unwrap_or(false)));
        App{
            config,
            dir: dir.to_string(),
            connection: Mutex::new(None),
            auto_reload,
            state: Mutex::new(AppState::Waiting),
        }
    }

    pub async fn get_state(&self) -> AppState{
        let state = &*self.state.lock().await;
        state.clone()
    }

    pub async fn set_running(&self) {
        *self.state.lock().await = AppState::Running;
    }

    pub async fn set_zombie(&self) {
        *self.state.lock().await = AppState::Zombie;
    }

    pub async fn start(&self) -> Result<(), UnicomError>{
        match *self.state.lock().await {
            AppState::Started|AppState::Running|AppState::Zombie => return Ok(()),
            _ => (),
        };

        let mut cmd = match &self.config.kind {
            AppType::Python { venv } => {
                let mut cmd = Command::new("unicom-python");
                cmd.arg(&self.dir);
                if let Some(venv) = venv{
                    cmd.arg(venv);
                }

                cmd
            },
        };
        *self.state.lock().await = AppState::Started;
        *self.connection.lock().await = Some(AppProcess::new(&mut cmd, self.config.name.clone())?);
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), UnicomError>{
        let mut t = self.connection.lock().await;
        if let Some(connection) = &mut *t{
            connection.stop().await?;
            *t = None;
        }
        drop(t);
        *self.state.lock().await = AppState::Stoped;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub enum AppType{
    Python{
        venv: Option<String>,
    }
}

#[derive(Debug, Deserialize)]
pub struct AppConfig{
    pub name: String, 
    pub kind: AppType,
    pub after: Option<String>,
    pub auto_reload: Option<bool>
}

impl AppConfig{
    pub async fn read_config(dir: &str) -> Result<AppConfig, UnicomError>{
        Ok(toml::from_str(&fs::read_to_string(Path::new(dir).join("config.toml")).await?)?)
    }
}


