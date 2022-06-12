use std::{sync::Arc, ffi::CString};

use chrono::{DateTime, Utc, Duration};
use hyper::{header::COOKIE, Request, Body};
use rand::Rng;
use regex::Regex;
use tokio::fs::OpenOptions;
use tokio::sync::Mutex;
use tokio::io::AsyncWriteExt;
use unicom_lib::error::{UnicomError, UnicomErrorKind};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum UserLevel {
    Admin,
    Root,
    Normal,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct User{
    name: String,
    level: UserLevel,
}

pub struct Session{
    pub id: String,
    user: std::sync::Mutex<Option<User>>,
    expire: DateTime<Utc>,
}

impl Session{
    fn new() -> Session{
        Session{
            id: format!("{:x}", rand::thread_rng().gen::<u64>()),
            user: std::sync::Mutex::new(None),
            expire: Utc::now().checked_add_signed(Duration::weeks(5)).unwrap(),
        }
    }

    fn has_expire(&self) -> bool{
        self.expire < Utc::now()
    }

    pub fn gen_cookies(&self) -> String{
        format!("sessionID={}; Expires={}; SameSite=Strict", self.id, self.expire.to_rfc2822())
    }

    fn set_user(&self, n_user: Option<User>){
        let mut user = self.user.lock().unwrap();
        *user = n_user;
    }

    pub fn get_user(&self) -> Option<User>{
        if let Some(user) = &*self.user.lock().unwrap(){
            return Some(user.clone())
        }
        None
    }


}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SessionJson{
    id: String,
    user: Option<User>,
    expire:String,
}

impl From<&Arc<Session>> for SessionJson {
    fn from(sess: &Arc<Session>) -> Self {
        let user = &*sess.user.lock().unwrap();
        SessionJson { 
            id: sess.id.clone(), 
            user: user.clone(), 
            expire: sess.expire.to_rfc2822() }
    }
}

impl Into<Arc<Session>> for SessionJson{
    fn into(self) -> Arc<Session> {
        Arc::new(Session { 
            id: self.id, 
            user: std::sync::Mutex::new(self.user), 
            expire: DateTime::parse_from_rfc2822(&self.expire).unwrap().into() })
    }
}



pub enum AuthenticationType{
    Unix,

}

pub struct SessionManager{
    path: String,
    sessions: Mutex<Vec<Arc<Session>>>,
    regex: Regex,
    a_type: AuthenticationType,

}

impl SessionManager{

    pub fn new(quick_load_path: &str) -> SessionManager{
        SessionManager{
            path: quick_load_path.to_string(),
            sessions: Mutex::new(Vec::new()),
            regex: Regex::new("sessionID=([0-9a-f]+);").unwrap(),
            a_type: AuthenticationType::Unix,
        }
    }

    pub async fn load(&self) -> Result<(), UnicomError>{
        let content = std::fs::read_to_string(&self.path)?;
        let mut data_set: Vec<SessionJson> = serde_json::from_str(&content)?;
        let mut sess = self.sessions.lock().await;
        sess.push(Arc::new(Session::new()));
        loop{
            sess.push( match data_set.pop(){
                Some(data) => data.into(),
                None => break,
            });
        }

        Ok(())
    }

    pub async fn save(&self){
        let mut data_set: Vec<SessionJson> = Vec::new();
        let sess = &mut *self.sessions.lock().await;
        for session in sess.iter(){
            data_set.push(session.into())
        }
        let data = serde_json::to_string(&data_set).unwrap();
        println!("data : {} {}", data, &self.path);
        let mut file = OpenOptions::new().write(true).create(true).open(&self.path).await.unwrap();
        file.write_all(data.as_bytes()).await.unwrap();
        file.sync_data().await.unwrap();

    }

    pub async fn create(&self) -> Arc<Session>{
        let mut sess = self.sessions.lock().await;
        sess.push(Arc::new(Session::new()));
        let ret = sess.last().unwrap().clone();
        drop(sess);
        //println!("session save");
        self.save().await;
        ret
    }

    async fn get(&self, id: &str) -> Option<Arc<Session>>{
        let sess = &mut *self.sessions.lock().await;
        let mut bad_index = Vec::new();
        let mut ret = None;

        for (index, session) in sess.iter().enumerate(){
            if session.has_expire(){
                bad_index.push(index);
                continue
            }
            if &session.id == id{
                ret = Some(session.clone());
                break;
            }
        }

        for index in bad_index.iter().rev(){
            sess.remove(*index);
        }

        ret
    }

    pub async fn parse_session(&self, parts: &Request<Body>) -> Option<Arc<Session>>{
        match parts.headers().get(COOKIE){
            Some(cookies) => {
                for data in self.regex.captures_iter(&(cookies.to_str().unwrap().to_string()+";")){
                    if let Some(session) = self.get(&data[1].to_string()).await{
                        return Some(session)
                    }
                    
                }
            },
            None => (),
        }
        None
    }

    pub async fn authentication(&self, id: &str, user_name: &str, password: &str) -> Result<(), UnicomError>{
        let session = match self.get(id).await{
            Some(session) => session,
            None => return Err(UnicomError::new(UnicomErrorKind::ParameterInvalid, &format!("session id not found {}", id))),
        };

        if user_name.len() == 0{
            session.set_user(None);
            self.save().await;
            return Ok(())
        }

        match self.a_type {
            AuthenticationType::Unix => {
                let hash = match shadow::Shadow::from_name(user_name){
                    Some(unix_user) => unix_user,
                    None => {
                        return Err(UnicomError::new(UnicomErrorKind::NotAllowed, "*User/password Not Allowed"))
                    },
                };
                //println!("password {:?}", &hash.password);
                if !pwhash::unix::verify(password, &hash.password){
                    return Err(UnicomError::new(UnicomErrorKind::NotAllowed, "User/*password Not Allowed"))
                }
                //println!("Ok");
                let mut level = UserLevel::Normal;
                let user = nix::unistd::User::from_name(user_name).unwrap().unwrap();
                //println!("USER {:?}", user);
                let name = CString::new(user.name).unwrap();
                for gid in nix::unistd::getgrouplist(&name, user.gid).unwrap(){
                    let group = nix::unistd::Group::from_gid(gid).unwrap().unwrap();
                    if group.name == "sudo"{
                        level = UserLevel::Admin
                    }
                    //println!("GROUP {:?}", group);
                }

                //println!("LEVEl {:?}", level);

                session.set_user(Some(User{
                    name: user_name.to_string(),
                    level,
                }));

                self.save().await;
            },
        };
        Ok(())
    }


}