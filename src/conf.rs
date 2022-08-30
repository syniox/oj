use clap::Parser;
use serde::{Serialize, Deserialize};

fn default_bind_address() -> String {
    "127.0.0.1".to_string()
}
fn default_bind_port() -> i32 {
    12345
}

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
pub struct Args {
    /// Read config file
    #[clap(short, long)]
    config: String,
    
    /// Flush data when started
    #[clap(short, long="flush-data")]
    flush: bool
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all="snake_case")]
pub enum ProblemType {
    Standard,
    Strict,
    Spj,
    DynamicRanking,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Problem {
    pub id: i32,
    pub name: String,
    pub r#type: ProblemType,
    #[serde(default)]
    pub misc: String,
    pub cases: Vec<Case>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Server {
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_bind_port")]
    pub bind_port: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Case {
    pub score: f64,
    pub input_file: String,
    pub answer_file: String,
    pub time_limit: i32,
    pub memory_limit: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Language {
    pub name: String, // Switch to enum?
    pub file_anme: String,
    pub command: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Conf {
    pub server: Server,
    pub problems: Vec<Problem>,
    pub languages: Vec<Language>,
}

impl Conf {
    pub fn parse() -> std::io::Result<Self> {
        let args = Args::parse();
        let json = std::fs::read_to_string(&args.config)?;
        let conf = serde_json::from_str(&json).unwrap();
        Ok(conf)
    }
}