use futures::StreamExt;
use libp2p::swarm::{keep_alive, NetworkBehaviour, Swarm, SwarmEvent};
use libp2p::{identity, PeerId, Multiaddr};
use libp2p::{mdns, gossipsub};
use std::error::Error;
use std::time::Duration;

mod state;

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>>{
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
    let topic = libp2p::gossipsub::IdentTopic::new("example");
    gossipsub.subscribe(&topic)?;

    let behaviour = MyBehavior {
        keep_alive: keep_alive::Behaviour::default(),
        mdns: mdns::async_io::Behaviour::new(mdns_config)?,
        gossipsub,
    };

    let mut swarm = Swarm::with_async_std_executor(transport, behaviour, local_peer_id);

    // system assign a port
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    if let Some(addr) = std::env::args().nth(1) {
        let remote: Multiaddr = addr.parse()?;
        swarm.dial(remote)?;
        print!("Dialed {addr}")
    }

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, ..} => println!("Listening on {address:?}") ,
            SwarmEvent::Behaviour(event) => {
                // println!("{event:?}");

                match event {
                    MyBehaviorEvent::Mdns(mdns::Event::Discovered(list)) => {
                        for (peer_id, _multiaddr) in list {
                            println!("mDNS discovered a new peer: {peer_id}");
                            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                        }
                    },
                    MyBehaviorEvent::Mdns(mdns::Event::Expired(list)) => {
                        for (peer_id, _multiaddr) in list {
                            println!("mDNS discovered peer expired: {peer_id}");
                            swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                        }
                    },
                    MyBehaviorEvent::Gossipsub(gossipsub::GossipsubEvent::Subscribed { peer_id, topic }) => {
                        println!("{peer_id:?} subscribed {topic:?}");
                        let msg = "Hello, world";
                        swarm.behaviour_mut().gossipsub.publish(topic, msg.as_bytes())?;
                    },
                    MyBehaviorEvent::Gossipsub(gossipsub::GossipsubEvent::Message { propagation_source, message_id, message }) => {
                        println!( "Got message: '{}' with id: {message_id} from peer: {propagation_source}", String::from_utf8_lossy(&message.data))
                    },
                    _ => println!("{event:?}")
                }
            },
            _ => {}
        }
    }
}

#[derive(NetworkBehaviour)]
pub struct MyBehavior {
    keep_alive: keep_alive::Behaviour,
    mdns: mdns::async_io::Behaviour,
    gossipsub: gossipsub::Gossipsub,
}