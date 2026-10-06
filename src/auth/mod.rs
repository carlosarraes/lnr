use crate::error::{AppError,Result};
use serde::{Serialize,Deserialize};
use std::path::PathBuf;
#[derive(Clone)]
pub struct Credential(String);
impl Credential {pub fn new(key:String)->Result<Self>{if key.trim().is_empty(){return Err(AppError::new("authentication","API key is empty"));}Ok(Self(key))} pub(crate) fn secret(&self)->&str{&self.0}}
impl std::fmt::Debug for Credential{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.write_str("Credential([redacted])")}}
#[derive(Default,Serialize,Deserialize)]
pub struct Config {pub default:Option<String>,#[serde(default)]pub workspaces:Vec<String>}
pub fn config_root()->Result<PathBuf>{
 if let Some(p)=std::env::var_os("XDG_CONFIG_HOME"){return Ok(PathBuf::from(p));}
 std::env::var_os("HOME").map(|p|PathBuf::from(p).join(".config")).ok_or_else(||AppError::new("configuration","Set HOME or XDG_CONFIG_HOME"))
}
pub fn resolve(workspace:Option<&str>)->Result<Credential>{
 if let Ok(key)=std::env::var("LINEAR_API_KEY") {
  if workspace.is_some(){return Err(AppError::input("--workspace conflicts with LINEAR_API_KEY; unset the variable to select stored credentials"));}
  return Credential::new(key);
 }
 let path=config_root()?.join("lnr/config.toml");
 let config:Config=match std::fs::read_to_string(path){Ok(s)=>toml::from_str(&s).map_err(|_|AppError::new("configuration","Invalid lnr config.toml"))?,Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Config::default(),Err(e)=>return Err(e.into())};
 let ws=workspace.or(config.default.as_deref()).ok_or_else(||AppError::new("authentication","Set LINEAR_API_KEY or run lnr auth import-linear --workspace SLUG"))?;
 if !config.workspaces.iter().any(|w|w==ws){return Err(AppError::new("authentication","Selected workspace is not configured"));}
 let key=entry("lnr",ws)?.get_password().map_err(|_|AppError::new("authentication","Cannot read workspace keyring entry; use LINEAR_API_KEY in headless sessions"))?;
 Credential::new(key)
}
pub fn entry(service:&str,ws:&str)->Result<keyring::Entry>{keyring::Entry::new(service,ws).map_err(|_|AppError::new("configuration","OS keyring unavailable"))}
pub mod import;
