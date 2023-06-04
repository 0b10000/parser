use std::any::Any;
use fnv::{FnvHashMap, FnvHashSet};
use main_error::MainError;
use std::env;
use std::fs;
use std::io::Write;
use std::ops::{Add, Sub};
use std::process::Output;
use std::ptr::null;
use itertools::Itertools;
use serde::{Serialize, Serializer};
use serde::ser::SerializeStruct;
use tf_demo_parser::demo::data::DemoTick;
use tf_demo_parser::demo::message::Message;
use tf_demo_parser::demo::packet::datatable::{ParseSendTable, SendTableName, ServerClass};
use tf_demo_parser::demo::parser::MessageHandler;
use tf_demo_parser::demo::sendprop::{SendPropIdentifier, SendPropName};
use tf_demo_parser::MessageType;
pub use tf_demo_parser::{Demo, DemoParser, Parse, ParseError, ParserState, Stream};
use tf_demo_parser::demo::header::Header;
use tf_demo_parser::demo::parser::analyser::Team;
use tf_demo_parser::demo::parser::gamestateanalyser::{GameState, GameStateAnalyser, Player};
use tf_demo_parser::demo::parser::DemoTicker;
use tf_demo_parser::demo::vector::Vector;

struct ViewAngles {
    pitch: f32,
    yaw: f32,
}

impl ViewAngles {
    fn new(pitch: f32, yaw: f32) -> Self {
        Self { pitch, yaw }
    }
}

impl Serialize for ViewAngles{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut s = serializer.serialize_struct("ViewAngles", 2)?;
        s.serialize_field("pitch", &self.pitch)?;
        s.serialize_field("yaw", &self.yaw)?;
        s.end()
    }
}

struct McdOutputFile {
    info: Header,
    ticks: Vec<PlayerTickInfo>
}
impl Serialize for McdOutputFile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut s = serializer.serialize_struct("McdOutputFile", 2)?;
        s.serialize_field("info", &self.info)?;
        s.serialize_field("ticks", &self.ticks)?;
        s.end()
    }
}

impl McdOutputFile {
    fn new() -> Self {
        Self {
            info: Header {
                demo_type: "".to_string(),
                version: 0,
                protocol: 0,
                server: "".to_string(),
                nick: "".to_string(),
                map: "".to_string(),
                game: "".to_string(),
                duration: 0.0,
                ticks: 0,
                frames: 0,
                signon: 0,
            },
            ticks: Vec::new(),
        }
    }
}

struct PlayerTickInfo {
    player: String,
    view_angles: ViewAngles,
    position: Vector,
}

impl Serialize for PlayerTickInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut s = serializer.serialize_struct("PlayerTickInfo", 3)?;
        s.serialize_field("player", &self.player)?;
        s.serialize_field("view_angles", &self.view_angles)?;
        s.serialize_field("position", &self.position)?;
        s.end()
    }
}

fn main() -> Result<(), MainError> {
    #[cfg(feature = "trace")]
    tracing_subscriber::fmt::init();
    let mut output = McdOutputFile::new();

    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        println!("1 argument required");
        return Ok(());
    }
    let path = args[1].clone();
    let file = fs::read(&path)?;
    let demo = Demo::owned(file);
    let (header, mut ticker) = DemoParser::new_all_with_analyser(demo.get_stream(), GameStateAnalyser::new()).ticker().unwrap();

    output.info = header;

    loop {
        match ticker.tick() {
            Ok(true) => {
                handle_tick(ticker.state(), &mut output);
                continue;
            }
            Ok(false) => {
                break;
            }
            Err(e) => {
                println!("Error: {:?}", e);
                break;
            }
        }
    }

    //write json to file
    let json = serde_json::to_string_pretty(&output)?;
    let mut file = fs::File::create("output.json")?;
    file.write_all(json.as_bytes())?;



    Ok(())
}

fn handle_tick(tick: &GameState, output: &mut McdOutputFile){
    println!("Tick: {}", u32::from(tick.tick));
    tick.players.iter().for_each(|player| {
        let player_info = player.info.as_ref().unwrap();
        output.ticks.push(PlayerTickInfo {
            player: player_info.name.clone(),
            view_angles: ViewAngles::new(player.pitch_angle, player.view_angle),
            position: player.position,
        });
    });
    return;
}
