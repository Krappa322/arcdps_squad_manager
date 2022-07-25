#![allow(non_snake_case)]

use chrono::{DateTime, FixedOffset};
use std::collections::HashMap;

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct Channel {
    pub channel_id: u32,
    pub channel_type: arcdps::ChannelType,
    pub subgroup: u8,
}

#[derive(Debug)]
pub struct ChatMessage {
    pub is_broadcast: bool,
    pub timestamp: DateTime<FixedOffset>,
    pub account_name: String,
    pub character_name: String,
    pub text: String,
}

fn split_message(pChatMessage: &arcdps::ChatMessageInfo) -> (Channel, ChatMessage) {
    (Channel {
        channel_id: pChatMessage.channel_id,
        channel_type: pChatMessage.channel_type,
        subgroup: pChatMessage.subgroup,
    },
    ChatMessage {
        is_broadcast: pChatMessage.is_broadcast,
        timestamp: pChatMessage.timestamp,
        account_name: pChatMessage.account_name.to_string(),
        character_name: pChatMessage.character_name.to_string(),
        text: pChatMessage.text.to_string(),
    })
}

pub struct ChatLog {
    channels: HashMap<Channel, Vec<ChatMessage>>,
}

impl ChatLog {
    pub fn new() -> Self { Self { channels: HashMap::new() } }

    pub fn add(&mut self, pChatMessage: &arcdps::ChatMessageInfo) {
        let (channel, msg) = split_message(pChatMessage);
        debug!("Received message {:?} into {:?}", msg, channel);

        let channel = self.channels.entry(channel).or_default();
        channel.push(msg);
    }

    pub fn get_all_messages(&self) -> Vec<(&Channel, &ChatMessage)> {
        let mut result: Vec<(&Channel, &ChatMessage)> = Vec::new();
        for c in self.channels.iter() {
            for m in c.1.iter() {
                result.push((c.0, m));
            }
        }

        result
    }
}