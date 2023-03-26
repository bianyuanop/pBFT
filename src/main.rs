use std::env;
use std::thread;
use futures::StreamExt;
use libp2p::gossipsub::error::PublishError;
use libp2p::swarm::{keep_alive, NetworkBehaviour, Swarm, SwarmEvent};
use libp2p::{identity, PeerId, Multiaddr};
use libp2p::{mdns, gossipsub::{self, IdentTopic}};
use state::Response;
use std::error::Error;
use std::time::Duration;
use state::{State, Phase};

mod state;
mod report;

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>>{

    let args: Vec<String> = env::args().collect();

    let mut state_id: u128 = 0;
    let mut f: u128 = 0;

    if args.len() < 2 {
        panic!("<id>, <f> are needed for initialization")
    } else {
        state_id = args[1].parse()?;
        f = args[2].parse()?;
    }

    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());

    println!("Local peer id: {local_peer_id:?}");

    let transport = libp2p::development_transport(local_key.clone()).await?;

    let mdns_config = mdns::Config {
        ttl: Duration::new(30, 0),
        query_interval: Duration::new(1, 0),
        enable_ipv6: false
    };

    let gossipsub_config = gossipsub::GossipsubConfig::default();
    let gossipsub_message_auth = gossipsub::MessageAuthenticity::Signed(local_key.clone());
    let mut gossipsub = gossipsub::Gossipsub::new(gossipsub_message_auth, gossipsub_config)?;

    let msg_topic = IdentTopic::new("example");
    gossipsub.subscribe(&msg_topic)?;

    let behaviour = MyBehavior {
        keep_alive: keep_alive::Behaviour::default(),
        mdns: mdns::async_io::Behaviour::new(mdns_config)?,
        gossipsub,
    };

    let mut swarm = Swarm::with_async_std_executor(transport, behaviour, local_peer_id);
    let mut state = State::new(state_id, f);

    // system assign a port
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, ..} => println!("Listening on {address:?}") ,
            SwarmEvent::Behaviour(event) => {
                // println!("{event:?}");

                match event {
                    MyBehaviorEvent::Mdns(mdns::Event::Discovered(list)) => {
                        for (peer_id, _multiaddr) in list {
                            // println!("mDNS discovered a new peer: {peer_id}");
                            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                        }
                    },
                    MyBehaviorEvent::Mdns(mdns::Event::Expired(list)) => {
                        for (peer_id, _multiaddr) in list {
                            // println!("mDNS discovered peer expired: {peer_id}");
                            swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                            state.on_remove_peer(peer_id);
                        }
                    },
                    MyBehaviorEvent::Gossipsub(gossipsub::GossipsubEvent::Subscribed { peer_id, topic }) => {
                        println!("{peer_id:?} subscribed {topic:?}");
                        let resp = state.on_new_peer(peer_id);

                        broadcast(&mut swarm, msg_topic.clone(), resp);
                    },
                    MyBehaviorEvent::Gossipsub(gossipsub::GossipsubEvent::Unsubscribed {
                        peer_id,
                        topic
                    }) => {
                        println!("{peer_id:?} unsubscribed {topic:?}");
                    },
                    MyBehaviorEvent::Gossipsub(gossipsub::GossipsubEvent::GossipsubNotSupported { peer_id }) => {
                        println!("gossipsub is not supported by {:?}", peer_id);
                    }
                    
                    MyBehaviorEvent::Gossipsub(gossipsub::GossipsubEvent::Message { propagation_source, message_id, message }) => {
                        // println!( "Got message: '{}' with id: {message_id} from peer: {propagation_source}", String::from_utf8_lossy(&message.data));

                        println!("[STATE | {:?}] {:?}", state.id, state.phase);
                        let decapsulate_msg: state::Message = serde_json::from_slice(&message.data)?;
                        println!("received msg: {:?}", decapsulate_msg);

                        let resp = state.on_message(decapsulate_msg, propagation_source);
                        println!("Response {:?}", resp);

                        broadcast(&mut swarm, msg_topic.clone(), resp);

                        println!();
                    },
                    _ => println!("{event:?}")
                }
            },
            _ => {}
        }
    }
}


fn broadcast(swarm: &mut Swarm<MyBehavior>, topic: IdentTopic, resp: Response) {
    match resp.r_type {
        state::ResponseType::Broadcast => {
            let mut retry = 3;

            while retry > 0 {
                let pub_res = swarm.behaviour_mut().gossipsub.publish(topic.clone(), serde_json::to_vec(&resp.m).unwrap()); 
                println!("[BROADCAST] {:?}", resp.m);
                match pub_res {
                    Err(e) => {
                        println!("Error {:?} occur", e);
                        thread::sleep(Duration::from_millis(1000));
                        retry = retry - 1;
                    },
                    _ => {
                        break;
                    }
                }
            }
        },
        state::ResponseType::DoNothing => {}
    };
}

#[derive(NetworkBehaviour)]
pub struct MyBehavior {
    keep_alive: keep_alive::Behaviour,
    mdns: mdns::async_io::Behaviour,
    gossipsub: gossipsub::Gossipsub,
}