use std::collections::HashMap;
use std::{default, hash};

use async_std::stream::Merge;
use libp2p::PeerId;
use libp2p::swarm::{Swarm};
use crate::MyBehavior;
use serde::{Serialize, Deserialize};


#[derive(Debug, Serialize, Deserialize, Default)]
pub enum MessageType {
    PrePrepare,
    Prepare,
    Commit,
    #[default]
    FinalCommit,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct  Message {
    pub id: u128,
    pub round: u128,
    pub m_type: MessageType,
    // parse payload by type
    pub payload: Vec<u8>,
}


#[derive(Debug, Default)]
pub enum ResponseType {
    Broadcast,
    #[default]
    DoNothing
}

#[derive(Debug, Default)]
pub struct Response {
    pub r_type: ResponseType,
    pub m: Message,
}

#[derive(PartialEq)]
pub enum Phase {
    NewRound,
    PrePrepared,
    Prepared,
    Committed,
    FinalCommitted,
    RoundChange,
}

pub struct State {
    pub id: u128,
    pub round: u128,
    pub phase: Phase,
    pub proposer: u128,
    pub prepare_pool: Vec<u128>,
    pub commit_pool: Vec<u128>,
    // PeerId to realId mapping
    pub peers: HashMap<PeerId, u128>,
    // pre-defined 
    pub f: u128,
}

impl State {
    pub fn new(id: u128, f: u128) -> Self {
        Self {
            id,
            f,
            round: 0,
            phase: Phase::NewRound,
            proposer: 0,
            prepare_pool: vec![],
            commit_pool: vec![],
            peers: HashMap::new(),
        }
    }

    pub fn on_message(&mut self, msg: Message) -> Response {
        if msg.round != self.round {
            return Response::default();
        }

        match msg.m_type {
            MessageType::PrePrepare => self.on_pre_prepare(msg),
            MessageType::Prepare => self.on_prepare(msg),
            MessageType::Commit => self.on_commit(msg),
            MessageType::FinalCommit => self.on_final_commit(msg),
        }
    }

    fn on_pre_prepare(&mut self, msg: Message) -> Response {
        if self.phase != Phase::NewRound {
            return Response::default();
        }
        // we are not verifying peer here since the messages are sent through p2p encrypted channel

        // TODO: excute the message, then verify the checksum


        // phase change
        self.phase = Phase::Prepared;

        let msg = Message {
            id: self.id,
            round: self.round,
            m_type: MessageType::Prepare,
            // further process needed
            payload: vec![]
        };

        let resp = Response {
            r_type: ResponseType::Broadcast,
            m: msg,
        };
        resp
    }

    fn on_prepare(&mut self, msg: Message) -> Response {
        if self.phase != Phase::Prepared {
            return Response::default();
        }
        // TODO: implement a peerid to real id pool

        self.prepare_pool.push(msg.id);

        Response::default()
    }

    fn on_commit(&mut self, msg: Message) -> Response {
        unimplemented!();
    }

    fn on_final_commit(&mut self, msg: Message) -> Response {
        unimplemented!();
    }

    fn on_round_change(&mut self, msg: Message) -> Response {
        unimplemented!();
    }

    fn on_new_round(&mut self, msg: Message) {
        unimplemented!();
    }
}
