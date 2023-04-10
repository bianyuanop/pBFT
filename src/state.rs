use std::{collections::{HashMap, VecDeque}, thread, time::{Duration, Instant}, fmt::Display, vec, task::Poll};

use async_std::stream::{
    Interval, interval, StreamExt, Merge
};
use futures::{Future, future};
use libp2p::PeerId;
use serde::{Serialize, Deserialize};


#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy, PartialEq)]
pub enum MessageType {
    #[default]
    PrePrepare,
    Prepare,
    Commit,
    RoundChange,
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
        if msg.round != self.round {
            return Response::default();
        }

        let resp = match msg.m_type {
            MessageType::PrePrepare => self.on_pre_prepare(msg),
            MessageType::Prepare => self.on_prepare(msg),
            MessageType::Commit => self.on_commit(msg),
            MessageType::RoundChange => self.on_round_change(msg),
        };

        // this means state change, refresh the `last_update_time`
        if resp.r_type == ResponseType::Broadcast {
            self.last_update_time = Instant::now();
        }

        resp
    }

    fn on_pre_prepare(&mut self, msg: Message) -> Response {
        if self.phase != Phase::NewRound {
            // do nothing
            return Response::default();
        }
        // we are not verifying peer here since the messages are sent through p2p encrypted channel

        // TODO: excute the message, then verify the checksum



        // phase change
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
        resp
    }

    fn on_prepare(&mut self, msg: Message) -> Response {
        if self.phase != Phase::Prepared {
            return Response::default();
        }
        // TODO: implement a peerid to real id pool

        self.prepare_pool.push(msg.id);
        
        if self.prepare_pool.len() as u128 > self.f * 2 + 1 {
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
        if self.phase != Phase::Committed {
            return Response::default();
        }

        self.commit_pool.push(msg.id);

        if self.commit_pool.len() as u128 > self.f * 2 + 1 {
            self.phase = Phase::FinalCommitted;

            return self.new_round();
        }


        Response::default()
    }

    fn on_round_change(&mut self, msg: Message) -> Response {

        if let Some(pool) = self.round_change_pool.get_mut(&msg.round) {
            pool.push(msg.id);
            if pool.len() as u128 >= 2*self.f + 1 {
                // update the round to be the newest one
                self.round = msg.round;

                println!("round change pool len {0}", self.round);
                return self.new_round()
            }
        } else {
            self.round_change_pool.insert(msg.id, vec![msg.id, ]);
        }
        Response::default()
    }

    pub fn on_new_peer(&mut self, peer: PeerId) -> Response {
        if self.peers.contains_key(&peer) {
            return Response::default()
        }

        // control initial nodes here 
        self.peers.insert(peer, self.f*3 + 2);

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
        
        // if the state is not changed from FinalCommitted, don't increse round
        if self.phase != Phase::NewRound {self.round = self.round + 1;} 

        self.commit_pool.clear();
        self.prepare_pool.clear();
        self.round_change_pool.clear();

        self.phase = Phase::NewRound;

        // TODO: pick up some transactions here 

        // become proposer
        if self.id == self.round % self.f {
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

            let msg = Message {
                id: self.id,
                round: self.round,
                m_type: MessageType::RoundChange,
                payload: vec![]
            };

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
