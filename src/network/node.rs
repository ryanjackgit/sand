use std::time::Duration;
use std::collections::HashMap;
use tokio::net::TcpStream;
use tokio::io::{AsyncRead, WriteHalf};
use tokio::codec::FramedRead;
use actix::prelude::*;
use actix_raft::{NodeId};
use tokio::sync::oneshot;
use std::marker::PhantomData;
use std::time::Instant;

use serde::{Serialize, de::DeserializeOwned};

use crate::network::{
    ClientNodeCodec,
    NodeRequest,
    NodeResponse,
    Network,
    network::AddConnectedNode,
  //  network::RemoveConnectedNode,
    network::NodeDisconnect,
    PeerConnected,
    remote::{
        RemoteMessage,
        RemoteMessageResult,
        SendRaftMessage,
    },
};

#[derive(PartialEq)]
pub enum NodeState {
    Registered,
    Connected
}

pub struct Node {
    id: NodeId,
    local_id: NodeId,
    mid: u64,
    state: NodeState,
    peer_addr: String,
    framed: Option<actix::io::FramedWrite<WriteHalf<TcpStream>, ClientNodeCodec>>,
    requests: HashMap<u64, oneshot::Sender<String>>,
    network: Addr<Network>,
   
}

impl Node {
    pub fn new(id: u64, local_id: NodeId, peer_addr: String, network: Addr<Network>) -> Self {
        Node {
            id: id,
            local_id: local_id,
            mid: 0,
            state: NodeState::Registered,
            peer_addr: peer_addr,
            framed: None,
            requests: HashMap::new(),
            network: network,
           
        }
    }

    fn connect(&mut self, ctx: &mut Context<Self>) {
        // node is already connected
        if self.state == NodeState::Connected {
            return ();
        }

        println!("Connecting to node #{}", self.id);

        let remote_addr = self.peer_addr.as_str().parse().unwrap();
        let conn = TcpStream::connect(&remote_addr)
            .map_err(|e| {
                println!("Error: {:?}", e);
            })
            .map(TcpConnect)
            .into_stream();

        ctx.add_message_stream(conn);
    }

    fn hb(&self, ctx: &mut Context<Self>) {

        ctx.run_later(Duration::new(1, 0), |act, ctx| {
            act.framed.as_mut().unwrap().write(NodeRequest::Ping);
            act.hb(ctx);
        });

/*
         ctx.run_interval(Duration::new(1, 0), |act, ctx| {

            if Instant::now().duration_since(act.hb) > Duration::new(10, 0) {
                println!("Client heartbeat failed, disconnecting!");
                ctx.stop();
            }

            // Reply heartbeat
            act.framed.as_mut().unwrap().write(NodeRequest::Ping);
        });
        */
    }
    
}

impl Actor for Node {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        ctx.notify(Connect);
    }

    fn stopped(&mut self, ctx: &mut Context<Self>) {
        println!("Node #{} disconnected", self.id);
        self.state = NodeState::Registered;
        // TODO: remove from network.nodes_connected

     //    self.network.do_send(RemoveConnectedNode(self.id));
        self.network.do_send(NodeDisconnect(self.id));
    }
}



#[derive(Message)]
struct TcpConnect(TcpStream);

#[derive(Message)]
struct Connect;

impl Handler<TcpConnect> for Node {
    type Result = ();

    fn handle(&mut self, msg: TcpConnect, ctx: &mut Context<Self>) {
         use crate::network::network::GetListenAddress;
        println!("Connected to remote node #{}", self.id);
        self.state = NodeState::Connected;
        let (r, w) = msg.0.split();
        Node::add_stream(FramedRead::new(r, ClientNodeCodec), ctx);
        self.framed = Some(actix::io::FramedWrite::new(w, ClientNodeCodec, ctx));

        self.network.do_send(PeerConnected(self.id));

        self.network.do_send(AddConnectedNode(self.id,self.peer_addr.clone()));

      
        fut::wrap_future::<_, Self>(self.network.send(GetListenAddress))
                            .map_err(move |err, _, _| panic!("Node {} not found"))
                            .and_then(move |addrss, act, ctx| {
              act.framed.as_mut().unwrap().write(NodeRequest::Join(act.local_id,addrss.unwrap().clone()));
              fut::ok(())
            }).wait(ctx);

    //    self.framed.as_mut().unwrap().write(NodeRequest::Join(self.local_id,self.peer_addr.clone()));
        self.hb(ctx);
    }
}

impl<M> Handler<SendRaftMessage<M>> for Node
where M: RemoteMessage + 'static,
      M::Result: Send + Serialize + DeserializeOwned
{
    type Result = RemoteMessageResult<M>;

    fn handle(&mut self, msg: SendRaftMessage<M>, _ctx: &mut Context<Self>) -> Self::Result {
        let (tx, rx) = oneshot::channel::<String>();

        if let Some(ref mut framed) = self.framed {
            self.mid += 1;
            self.requests.insert(self.mid, tx);

            let body = serde_json::to_string::<M>(&msg.0).unwrap();
            let request = NodeRequest::Message(self.mid, M::type_id(), body);
            framed.write(request);
        }

        RemoteMessageResult{rx: rx, m: PhantomData}
    }
}

impl Handler<Connect> for Node {
    type Result = ();

    fn handle(&mut self, _msg: Connect, ctx: &mut Context<Self>) {
        ctx.run_later(Duration::new(2, 0), |act, ctx| {
            act.connect(ctx);
            ctx.notify(Connect);
        });
    }
}

impl actix::io::WriteHandler<std::io::Error> for Node {}

impl StreamHandler<NodeResponse, std::io::Error> for Node {
    fn handle(&mut self, msg: NodeResponse, _ctx: &mut Context<Self>) {
        match msg {
            NodeResponse::Result(mid, data) => {
                if let Some(tx) = self.requests.remove(&mid) {
                    let _ = tx.send(data);
                }
            },
            NodeResponse::Ping => {
                // println!("Client got Ping from {}", self.id);
               // self.hb = Instant::now();
              },
            _ => ()
        }
    }
}

   use crate::network::network::SendToRaft;

impl Handler<SendToRaft> for Node {
 
    type Result = Result<String, ()>;

    fn handle(&mut self, msg: SendToRaft, _ctx: &mut Context<Self>) -> Self::Result {
                 if let Some(ref mut framed) = self.framed {
            self.mid += 1;
         //   self.requests.insert(self.mid, tx);

         //   let body = serde_json::to_string(&msg.0).unwrap();
            let request = NodeRequest::Message(self.mid, msg.0, msg.1);
            framed.write(request);
            return   Ok("yes".to_string());
        }
        Err(())

    }
}


