use async_std::stream::Merge;
use libp2p::PeerId;
use libp2p::swarm::{Swarm};
use crate::MyBehavior;


pub enum MessageType {
    PrePrepare,
    Prepare,
    Commit,
    FinalCommit,
    NewRound,
}

impl MessageType {
    pub fn 
}

pub struct  Message {
    pub id: u128,
    pub round: u128,
    pub m_type: MessageType,
    // parse payload by type
    pub payload: Vec<u8>,
}

impl Message {
    pub fn serialize(&self) -> Vec<u8> {
        unimplemented!()
    } 

    pub fn from(data: Vec<u8>) -> Self {
        unimplemented!()
    }
}

pub enum ResponseType {
    Broadcast,
    DoNothing
}

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
    pub prepare_pool: Vec<PeerId>,
    pub commit_pool: Vec<PeerId>,
    pub peers: Vec<PeerId>,
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
            peers: vec![],
        }
    }

    pub fn on_message(&self, msg: Message) {
        match msg.m_type {
            MessageType::NewRound => self.on_new_round(msg),
            MessageType::PrePrepare => self.on_pre_prepare(msg),
            MessageType::Prepare => self.on_prepare(msg),
            MessageType::Commit => self.on_commit(msg),
            MessageType::FinalCommit => self.on_final_commit(msg),
        }
    }

    fn on_pre_prepare(&self, msg: Message) {
        if self.phase != Phase::NewRound {
            return; 
        }
        // operations here for the calculation in the msg

    }

    fn on_prepare(&self, msg: Message) {

    }

    fn on_commit(&self, msg: Message) {

    }

    fn on_final_commit(&self, msg: Message) {

    }

    fn on_round_change(&self, msg: Message) {

    }

    fn on_new_round(&self, msg: Message) {

    }

    fn broadcast(&self, msg: Message) {
    }
}
