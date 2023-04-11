use std::{collections::{HashMap, VecDeque}, thread, time::{Duration, Instant}, fmt::Display, vec, task::Poll};
use colored::Colorize;
extern crate libp2p;

use async_std::stream::{
    Interval, interval, StreamExt, Merge
};
use futures::{Future, future};
use libp2p::PeerId;
use serde::{Serialize, Deserialize, ser::{SerializeStruct, SerializeSeq}};


#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy, PartialEq)]
pub enum MessageType {
    #[default]
    PrePrepare,
    Prepare,
    Commit,
    RoundChange,
    NewRound
}

impl Display for MessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self) 
    }
}

pub struct Transaction {
    pub opcode: u128,
    pub payload: Vec<u8>
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct  Message {
    pub id: u128,
    pub round: u128,
    pub m_type: MessageType,
    // parse payload by type
    pub payload: Vec<u8>,
}

#[derive(Debug, Default, Clone)]
pub struct NewRoundPayload {
    // will be deserilized into PeerId, here for simplicity we implement this in function
    signs: Vec<PeerId>,
    block: Vec<u8>,
}

#[derive(Debug, Default, PartialEq)]
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

#[derive(PartialEq, Debug, Eq)]
pub enum Phase {
    // only issued by next proposer
    NewRound,
    PrePrepared,
    Prepared,
    Committed,
    FinalCommitted,
    RoundChange,
}

impl Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self) 
    }
}

pub struct State {
    pub id: u128,
    pub round: u128,
    pub phase: Phase,
    pub proposer: u128,
    pub pre_prepare_msg: Option<Message>,
    pub prepare_pool: Vec<u128>,
    pub commit_pool: Vec<u128>,
    // different from above two since there may be different next round will be used
    pub round_change_pool: HashMap<u128, Vec<u128>>,
    // PeerId to realId mapping
    pub peers: HashMap<PeerId, u128>,
    // pre-defined 
    pub f: u128,
    pub tx_pool: Vec<Transaction>,

    // main thread will automatically fetch messages here to send
    pub message2send: VecDeque<Message>,
    pub last_update_time: Instant,

    timeout: Duration,
    started: bool
}

impl State {
    pub fn new(id: u128, f: u128) -> Self {
        Self {
            id,
            f,
            round: 0,
            phase: Phase::NewRound,
            proposer: 0,
            pre_prepare_msg: None,
            prepare_pool: vec![],
            commit_pool: vec![],
            round_change_pool: HashMap::new(),
            peers: HashMap::new(),
            tx_pool: vec![],
            message2send: VecDeque::new(),
            last_update_time: Instant::now(),
            timeout: Duration::from_secs(5),
            started: false
        }
    }

    pub fn is_timeout(&self) -> bool {
        if self.last_update_time.elapsed() > Duration::from_secs(2) {
            true
        } else {
            false
        }
    }

    pub fn on_message(&mut self, msg: Message, peer: PeerId) -> Response {

        if msg.round != self.round && msg.m_type != MessageType::NewRound && msg.m_type != MessageType::RoundChange {
            return Response::default();
        } 

        let resp = match msg.m_type {
            MessageType::PrePrepare => self.on_pre_prepare(msg),
            MessageType::Prepare => self.on_prepare(msg),
            MessageType::Commit => self.on_commit(msg),
            MessageType::RoundChange => self.on_round_change(msg),
            MessageType::NewRound => self.on_new_round(msg),
        };

        // this means state change, refresh the `last_update_time`
        if resp.r_type == ResponseType::Broadcast {
            self.last_update_time = Instant::now();
        }

        resp
    }

    fn on_pre_prepare(&mut self, msg: Message) -> Response {
        if self.phase != Phase::NewRound {
            return Response::default();
        }

        // we are not verifying peer here since the messages are sent through p2p encrypted channel

        // TODO: excute the message, then verify the checksum
        self.phase = Phase::Prepared;

        self.prepare_pool.push(self.id);
        // push proposer's id
        self.prepare_pool.push(msg.id);

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

        return resp;
    }

    fn on_prepare(&mut self, msg: Message) -> Response {
        if self.phase != Phase::Prepared  {
            return Response::default();
        }
        // TODO: implement a peerid to real id pool

        self.prepare_pool.push(msg.id);
        
        if self.prepare_pool.len() as u128 >= self.f * 2 + 1 {
            self.phase = Phase::Committed;

            // commit self
            self.commit_pool.push(self.id);

            let msg = Message {
                id: self.id,
                round: self.round,
                m_type: MessageType::Commit,
                payload: vec![],
            };
            return Response {
                r_type: ResponseType::Broadcast,
                m: msg
            };
        }

        Response::default()
    }

    fn on_commit(&mut self, msg: Message) -> Response {
        if self.phase != Phase::Committed && self.phase != Phase::Prepared {
            return Response::default();
        }

        self.commit_pool.push(msg.id);

        println!("Commit pool len {0}", self.commit_pool.len());

        if self.commit_pool.len() as u128 >= self.f * 2 + 1 {
            self.phase = Phase::FinalCommitted;

            return self.new_round();
        }


        Response::default()
    }

    fn on_round_change(&mut self, msg: Message) -> Response {
        if let Some(pool) = self.round_change_pool.get_mut(&msg.round) {
            if pool.contains(&msg.id) {
                return Response::default(); 
            }

            pool.push(msg.id);
            println!("round change pool({0}) len {1}", msg.round, pool.len());
            if pool.len() as u128 >= 2*self.f + 1 {
                return self.round_change()
            }
        } else {
            self.round_change_pool.insert(msg.round, vec![msg.id, ]);
        }

        Response::default()
    }

    fn round_change(&mut self) -> Response {
        if (self.peers.len() as u128) < self.f * 2 + 1 {
            return Response::default();
        }

        self.phase = Phase::NewRound;
        self.round += 1;

        self.prepare_pool.clear();
        self.commit_pool.clear();
        self.round_change_pool.clear();


        // is proposer, issue NewRound, add signatures of accepted round change peers
        if self.id == self.round % (self.f * 2 + 1) {
            println!("{}", "[IMPO] proposer".red());
            self.phase = Phase::Prepared;

            self.prepare_pool.push(self.id);

            let msg = Message {
                m_type: MessageType::NewRound,
                id: self.id,
                round: self.round,
                payload: vec![],
            };

            let resp = Response {r_type: ResponseType::Broadcast, m: msg};

            return resp;
        }

        Response::default()
    }

    fn on_new_round(&mut self, msg: Message) -> Response {
        // TODO: verify signatures here 

        // execute transaction as pre_prepare

        // clear old pool
        self.prepare_pool.clear();
        self.commit_pool.clear();

        self.phase = Phase::Prepared;

        self.prepare_pool.push(self.id);
        self.prepare_pool.push(msg.id);

        let msg = Message {
            id: self.id,
            round: self.round,
            m_type: MessageType::Prepare,
            payload: vec![],
        };

        let resp = Response {
            r_type: ResponseType::Broadcast,
            m: msg
        };

        resp
    }

    pub fn on_new_peer(&mut self, peer: PeerId) -> Response {
        if self.peers.contains_key(&peer) {
            return Response::default()
        }

        // control initial nodes here 
        self.peers.insert(peer, self.f*3 + 2);

        println!("peers len: {0}", self.peers.len());

        if self.peers.len() as u128 >= 3 * self.f  && !self.started {
            // start up
            self.started = true;
            self.new_round()
        } else {
            Response::default()
        }
    }

    pub fn on_remove_peer(&mut self, peer: PeerId) -> Response {
        self.peers.remove(&peer);

        Response::default()
    }

    fn new_round(&mut self) -> Response {
        if (self.peers.len() as u128) < self.f * 2 + 1 {
            return Response::default()
        }

        if (self.phase != Phase::NewRound) && (self.phase != Phase::FinalCommitted ) && (self.phase != Phase::RoundChange) {
            return Response::default()
        } 
        
        // except start up, increase round number
        if self.phase != Phase::NewRound {self.round = self.round + 1;} 

        self.commit_pool.clear();
        self.prepare_pool.clear();
        self.round_change_pool.clear();

        self.phase = Phase::NewRound;

        println!(":::NEWROUND::: {0}", self.round);

        // TODO: pick up some transactions here 

        // become proposer
        if self.id == self.round % (self.f * 2 + 1) {
            println!("{:?} is proposer", self.id);

            self.prepare_pool.push(self.id);
            self.phase = Phase::Prepared;

            let msg = Message {
                id: self.id,
                round: self.round,
                m_type: MessageType::PrePrepare,
                payload: vec![]
            };


            // sleep 1s to avoid too fast of processing
            thread::sleep(Duration::from_millis(1000));

            return Response {
                r_type: ResponseType::Broadcast,
                m: msg
            }
        }
        println!("not proposer");

        Response::default()
    }

    pub fn check_timeout(&mut self) -> Response {
        if !self.started {
            return Response::default()
        }

        if self.last_update_time.elapsed() > self.timeout {
            // update the last update time
            self.last_update_time = Instant::now();

            // change state to RoundChange, clear prepare and commit pool
            self.phase = Phase::RoundChange;

            let msg = Message {
                id: self.id,
                round: self.round,
                m_type: MessageType::RoundChange,
                payload: vec![]
            };

            self.on_round_change(msg.clone());

            let resp = Response {
                r_type: ResponseType::Broadcast,
                m: msg
            };

            resp
        } else {
            Response::default()
        }
    }
}
