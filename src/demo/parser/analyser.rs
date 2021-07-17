use std::collections::{BTreeMap, HashMap};

use serde::{ser::SerializeMap, Deserialize, Serialize, Serializer};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::demo::gameevent_gen::{
    GameEvent, PlayerDeathEvent, PlayerSpawnEvent, TeamPlayRoundWinEvent,
};
use crate::demo::message::packetentities::EntityId;
use crate::demo::message::usermessage::{ChatMessageKind, SayText2Message, UserMessage};
use crate::demo::message::{Message, MessageType};
use crate::demo::packet::stringtable::StringTableEntry;
use crate::demo::parser::handler::{BorrowMessageHandler, MessageHandler};
use crate::demo::vector::Vector;
use crate::{ParserState, ReadResult, Stream};
use num_enum::TryFromPrimitive;
use std::convert::TryFrom;
use std::ops::{Index, IndexMut};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMassage {
    pub kind: ChatMessageKind,
    pub from: String,
    pub text: String,
    pub tick: u32,
}

impl ChatMassage {
    pub fn from_message(message: &SayText2Message, tick: u32) -> Self {
        ChatMassage {
            kind: message.kind,
            from: message.from.clone().unwrap_or_default(),
            text: message.text.clone(),
            tick,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq, Eq, Hash, TryFromPrimitive)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum Team {
    Other = 0,
    Spectator = 1,
    Red = 2,
    Blue = 3,
}

impl Team {
    pub fn new<U>(number: U) -> Self
    where
        u8: TryFrom<U>,
    {
        Team::try_from(u8::try_from(number).unwrap_or_default()).unwrap_or_default()
    }

    pub fn is_player(&self) -> bool {
        *self == Team::Red || *self == Team::Blue
    }
}

impl Default for Team {
    fn default() -> Self {
        Team::Other
    }
}

#[derive(
    Debug, Clone, Serialize_repr, Deserialize_repr, Copy, PartialEq, Eq, Hash, TryFromPrimitive,
)]
#[repr(u8)]
pub enum Class {
    Other = 0,
    Scout = 1,
    Sniper = 2,
    Soldier = 3,
    Demoman = 4,
    Medic = 5,
    Heavy = 6,
    Pyro = 7,
    Spy = 8,
    Engineer = 9,
}

impl Class {
    pub fn new<U>(number: U) -> Self
    where
        u8: TryFrom<U>,
    {
        Class::try_from(u8::try_from(number).unwrap_or_default()).unwrap_or_default()
    }
}

impl Default for Class {
    fn default() -> Self {
        Class::Other
    }
}

#[derive(Default, Debug, Eq, PartialEq, Deserialize, Clone)]
#[serde(from = "HashMap<Class, u8>")]
pub struct ClassList([u8; 10]);

impl ClassList {
    /// Get an iterator for all classes played and the number of spawn on the class
    pub fn iter(&self) -> impl Iterator<Item = (Class, u8)> + '_ {
        self.0
            .iter()
            .copied()
            .enumerate()
            .map(|(class, count)| (Class::new(class), count))
            .filter(|(_, count)| *count > 0)
    }

    /// Get an iterator for all classes played and the number of spawn on the class, sorted by the number of spawns
    pub fn sorted(&self) -> impl Iterator<Item = (Class, u8)> {
        let mut classes = self.iter().collect::<Vec<(Class, u8)>>();
        classes.sort_by(|a, b| a.1.cmp(&b.1).reverse());
        classes.into_iter()
    }
}

#[test]
fn test_classlist_sorted() {
    let list = ClassList([0, 1, 5, 0, 0, 3, 0, 0, 0, 0]);
    assert_eq!(
        list.sorted().collect::<Vec<_>>(),
        &[(Class::Sniper, 5), (Class::Medic, 3), (Class::Scout, 1)]
    )
}

impl Index<Class> for ClassList {
    type Output = u8;

    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn index(&self, class: Class) -> &Self::Output {
        &self.0[class as u8 as usize]
    }
}

impl IndexMut<Class> for ClassList {
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn index_mut(&mut self, class: Class) -> &mut Self::Output {
        &mut self.0[class as u8 as usize]
    }
}

impl Serialize for ClassList {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let count = self.0.iter().filter(|c| **c > 0).count();
        let mut classes = serializer.serialize_map(Some(count))?;
        for (class, count) in self.0.iter().copied().enumerate() {
            if count > 0 {
                classes.serialize_entry(&class, &count)?;
            }
        }

        classes.end()
    }
}

impl From<HashMap<Class, u8>> for ClassList {
    fn from(map: HashMap<Class, u8>) -> Self {
        let mut classes = ClassList::default();

        for (class, count) in map.into_iter() {
            classes[class] = count;
        }

        classes
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct UserId(pub u8);

impl From<u32> for UserId {
    fn from(int: u32) -> Self {
        UserId((int & 255) as u8)
    }
}

impl From<u16> for UserId {
    fn from(int: u16) -> Self {
        UserId((int & 255) as u8)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Spawn {
    pub user: UserId,
    pub class: Class,
    pub team: Team,
    pub tick: u32,
}

impl Spawn {
    pub fn from_event(event: &PlayerSpawnEvent, tick: u32) -> Self {
        Spawn {
            user: UserId::from(event.user_id),
            class: Class::new(event.class),
            team: Team::new(event.team),
            tick,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub classes: ClassList,
    pub name: String,
    pub user_id: UserId,
    pub steam_id: String,
    #[serde(skip)]
    pub entity_id: EntityId,
    pub team: Team,
}

impl From<crate::demo::data::UserInfo> for UserInfo {
    fn from(info: crate::demo::data::UserInfo) -> Self {
        UserInfo {
            classes: ClassList::default(),
            name: info.name,
            user_id: info.user_id,
            steam_id: info.steam_id,
            entity_id: info.entity_id,
            team: Team::default(),
        }
    }
}

impl PartialEq for UserInfo {
    fn eq(&self, other: &UserInfo) -> bool {
        self.classes == other.classes
            && self.name == other.name
            && self.user_id == other.user_id
            && self.steam_id == other.steam_id
            && self.team == other.team
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Death {
    pub weapon: String,
    pub victim: UserId,
    pub assister: Option<UserId>,
    pub killer: UserId,
    pub tick: u32,
}

impl Death {
    pub fn from_event(event: &PlayerDeathEvent, tick: u32) -> Self {
        let assister = if event.assister < (16 * 1024) {
            Some(UserId::from(event.assister))
        } else {
            None
        };
        Death {
            assister,
            tick,
            killer: UserId::from(event.attacker),
            weapon: event.weapon.clone(),
            victim: UserId::from(event.user_id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Round {
    pub winner: Team,
    pub length: f32,
    pub end_tick: u32,
}

impl Round {
    pub fn from_event(event: &TeamPlayRoundWinEvent, tick: u32) -> Self {
        Round {
            winner: Team::new(event.team),
            length: event.round_time,
            end_tick: tick,
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct World {
    pub boundary_min: Vector,
    pub boundary_max: Vector,
}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Analyser {
    state: MatchState,
    user_id_map: HashMap<EntityId, UserId>,
}

impl MessageHandler for Analyser {
    type Output = MatchState;

    fn does_handle(message_type: MessageType) -> bool {
        matches!(
            message_type,
            MessageType::GameEvent | MessageType::UserMessage | MessageType::ServerInfo
        )
    }

    fn handle_message(&mut self, message: &Message, tick: u32) {
        if self.state.start_tick == 0 {
            self.state.start_tick = tick;
        }
        match message {
            Message::ServerInfo(message) => {
                self.state.interval_per_tick = message.interval_per_tick
            }
            Message::GameEvent(message) => self.handle_event(&message.event, tick),
            Message::UserMessage(message) => self.handle_user_message(&message, tick),
            _ => {}
        }
    }

    fn handle_string_entry(&mut self, table: &str, _index: usize, entry: &StringTableEntry) {
        if table == "userinfo" {
            let _ = self.parse_user_info(
                entry.text.as_ref().map(|s| s.as_ref()),
                entry.extra_data.as_ref().map(|data| data.data.clone()),
            );
        }
    }

    fn into_output(self, _state: &ParserState) -> Self::Output {
        self.state
    }
}

impl BorrowMessageHandler for Analyser {
    fn borrow_output(&self, _state: &ParserState) -> &Self::Output {
        &self.state
    }
}

impl Analyser {
    pub fn new() -> Self {
        Self::default()
    }

    fn handle_user_message(&mut self, message: &UserMessage, tick: u32) {
        if let UserMessage::SayText2(text_message) = message {
            if text_message.kind == ChatMessageKind::NameChange {
                if let Some(from) = text_message.from.clone() {
                    self.change_name(from, text_message.text.clone());
                }
            } else {
                self.state
                    .chat
                    .push(ChatMassage::from_message(text_message, tick));
            }
        }
    }

    fn change_name(&mut self, from: String, to: String) {
        if let Some(user) = self.state.users.values_mut().find(|user| user.name == from) {
            user.name = to;
        }
    }

    fn handle_event(&mut self, event: &GameEvent, tick: u32) {
        const WIN_REASON_TIME_LIMIT: u8 = 6;

        match event {
            GameEvent::PlayerDeath(event) => self.state.deaths.push(Death::from_event(event, tick)),
            GameEvent::PlayerSpawn(event) => {
                let spawn = Spawn::from_event(event, tick);
                if let Some(user_state) = self.state.users.get_mut(&spawn.user) {
                    user_state.classes[spawn.class] += 1;
                    user_state.team = spawn.team;
                }
            }
            GameEvent::TeamPlayRoundWin(event) => {
                if event.win_reason != WIN_REASON_TIME_LIMIT {
                    self.state.rounds.push(Round::from_event(event, tick))
                }
            }
            _ => {}
        }
    }

    fn parse_user_info(&mut self, text: Option<&str>, data: Option<Stream>) -> ReadResult<()> {
        if let Some(user_info) = crate::demo::data::UserInfo::parse_from_string_table(text, data)? {
            self.state
                .users
                .entry(user_info.user_id)
                .and_modify(|info| {
                    info.entity_id = user_info.entity_id;
                })
                .or_insert_with(|| user_info.into());
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct MatchState {
    pub chat: Vec<ChatMassage>,
    pub users: BTreeMap<UserId, UserInfo>,
    pub deaths: Vec<Death>,
    pub rounds: Vec<Round>,
    pub start_tick: u32,
    pub interval_per_tick: f32,
}
