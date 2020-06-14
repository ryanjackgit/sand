use actix::prelude::*;
use actix_raft::{ messages, NodeId, RaftMetrics};
use actix_raft::admin::{InitWithConfig, ProposeConfigChange, ProposeConfigChangeError};
use log::debug;
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};
use tokio::timer::Delay;
use actix_web::client::Client;
use serde::{de::DeserializeOwned, Serialize, Deserialize};

use crate::network::{Listener, MsgTypes, Node, NodeSession, remote::{SendRaftMessage}};
use crate::raft::{storage,storage::MemoryStorageData, RaftNode};
use crate::utils::generate_node_id;

pub type Payload = messages::ClientPayload<storage::MemoryStorageData, storage::MemoryStorageResponse, storage::MemoryStorageError>;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum NetworkState {
   Initialized,
    SingleNode,
    Cluster,
}

pub struct Network {
    id: NodeId,
    address: Option<String>,
    raft: Option<RaftNode>,
    static_peers:HashMap<NodeId, String>,
    nodes: HashMap<NodeId, Addr<Node>>,
  pub  isolated_nodes: Vec<NodeId>,

    all_connected_nodes:HashMap<NodeId, String>,
    register_server:String,

    nodes_connected: Vec<NodeId>,

    listener: Option<Addr<Listener>>,
    state: NetworkState,
    pub metrics: Option<RaftMetrics>,
    sessions: HashMap<NodeId, Addr<NodeSession>>,
    join_mode:bool,
}

impl Network {
    pub fn new(register_server:String) -> Network {
        Network {
            id: 0,
            address: None,
            static_peers: HashMap::new(),
            nodes: HashMap::new(),
            raft: None,
            listener: None,
            all_connected_nodes:HashMap::new(),
            register_server:register_server,
            nodes_connected: Vec::new(),
            isolated_nodes: Vec::new(),
            state: NetworkState::Initialized,
            metrics: None,
            sessions: HashMap::new(),
            join_mode:false,
        }
    }

    /// set peers
    pub fn peers(&mut self, peers: Vec<String>) {
        for peer in peers.into_iter() {
            let id = generate_node_id(&peer);
            self.static_peers.insert(id,peer);
        }
    }

    /// register a new node to the network
    pub fn register_node(&mut self, peer_addr: &str, addr: Addr<Self>) {

        let id = generate_node_id(peer_addr);
        let local_id = self.id;
        let peer_addr = peer_addr.to_owned();

         let network_address = self.address.as_ref().unwrap().clone();

        if peer_addr == *network_address {
            return ();
        }

        self.restore_node(id);

      
        if !self.nodes.contains_key(&id) {
        println!("the id --{} is register_node",id);
        let node =Node::new(id, local_id, peer_addr, addr).start();
        self.nodes.insert(id, node);

        }
    }

    /// get a node from the network by its id
    pub fn get_node(&self, id: NodeId) -> Option<&Addr<Node>> {
        self.nodes.get(&id)
    }

    pub fn isolate_node(&mut self, id: NodeId) {
        if let Some((idx, _)) = self.isolated_nodes.iter().enumerate().find(|(_, e)| *e == &id) {
            return ();
        }

       
        // self.isolated_nodes.push(id);
    }

    /// Restore the network of the specified node.
    pub fn restore_node(&mut self, id: NodeId) {
        if let Some((idx, _)) = self.isolated_nodes.iter().enumerate().find(|(_, e)| *e == &id) {
            debug!("Restoring network for node {}.", &id);
            self.isolated_nodes.remove(idx);
        }
    }

    pub fn listen(&mut self, address: &str) {
        self.address = Some(address.to_owned());
        self.id = generate_node_id(address);
    }
}

impl Actor for Network {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        let network_address = self.address.as_ref().unwrap().clone();

        let cluster_nodes_route = format!("http://{}/cluster/nodes", self.register_server.as_str());
         let cluster_state_route = format!("http://{}/cluster/state", self.register_server.as_str());

        println!("Listening on {}", network_address);
        println!("Local node id: {}", self.id);
        let listener_addr = Listener::new(network_address.as_str(), ctx.address().clone());
        self.listener = Some(listener_addr);
        self.nodes_connected.push(self.id);

       //  let peers = self.peers.clone();

   
             let mut client = Client::default();

           fut::wrap_future::<_, Self>(client.get(cluster_state_route).send())
            .map_err(|_, _, _| ())
            .and_then(move |res, _, _| {
                let mut res = res;
                fut::wrap_future::<_, Self>(res.body()).then(move |resp, _, _| {
                    if let Ok(body) = resp {
                        let state = serde_json::from_slice::<Result<NetworkState, ()>>(&body)
                            .unwrap().unwrap();

                        if state == NetworkState::Cluster {
                         // 从网络中获得所有已有的nodeid  address
                            return fut::Either::A(fut::wrap_future::<_, Self>(client.get(cluster_nodes_route).send())
                                                  .map_err(|e, _, _| println!("HTTP Cluster Error {:?}", e))
                                                  .and_then(|res, act, _| {
                                                      let mut res = res;
                                                      fut::wrap_future::<_, Self>(res.body()).then(|resp, act, _| {
                                                          if let Ok(body) = resp {
                                                              let mut nodes = serde_json::from_slice::<Result<HashMap<NodeId, String>, ()>>(&body)
                                                                  .unwrap().unwrap();

                                                             act.all_connected_nodes = nodes;

                                                         //     act.nodes_info = nodes;
                                                              act.join_mode = true;
                                                          }

                                                          fut::ok(())
                                                      })
                                                  })
                                                  .and_then(|_, _, _| fut::ok(()))
                            );
                        }
                    }

                    fut::Either::B(fut::ok(()))
                })
            })
            .map_err(|e, _, _| println!("not NetworkState::Cluster,HTTP Cluster Error {:?}", e))
            .and_then(move |_, act, ctx| {
              //  let nodes = act.nodes_info.clone();
              let mut nodes= act.all_connected_nodes.clone();
              nodes.extend(act.static_peers.clone());

       //          nodes.insert(7220731670040962359,"127.0.0.1:8001".to_string());
                for (id, info) in &nodes {
                    act.register_node(info, ctx.address().clone());
                }

                fut::ok(())
            })
            .spawn(ctx);



            //发现5秒后本节点已连接的节点,并开始初始化，随后把本节点动态加入网络
            fut::wrap_future::<_, Self>(ctx.address().send(DiscoverNodes))
            .map_err(|err, _, _| panic!(err))
            .and_then(|res, act, ctx| {
                let res = res.unwrap();
                let nodes = res.0;
                let join_mode = res.1;

                fut::wrap_future::<_, Self>(ctx.address().send(InitRaft{
                    nodes:nodes.clone(),
                    net:ctx.address(),
                    join_mode:join_mode,
                    }))
                    .map_err(|err, _, _| panic!(err))
                    .and_then(move |_, act, ctx| {
                        let mut client = Client::default();
                        let cluster_join_route = format!("http://{}/cluster/join", act.register_server.as_str());

                 //       act.app_net.do_send(SetClusterState(NetworkState::Cluster));
                       ctx.address().do_send(SetClusterState(NetworkState::Cluster));

                     
                        if join_mode {
                            fut::wrap_future::<_, Self>(client.put(cluster_join_route)
                                                        .header("Content-Type", "application/json")
                                                        .send_json(&act.id))
                                .map_err(|err, _, _| println!("Error joining cluster {:?}", err))
                                .and_then(|res, act, ctx| {
                                    fut::wrap_future::<_, Self>(Delay::new(Instant::now() + Duration::from_secs(3)))
                                        .map_err(|_, _, _| ())
                                        .and_then(|_, act, ctx| {
                                          //   act.raft.do_send(AddNode(act.id));
                                            println!("web已发出命令clister join ---{}",act.id);
                                            fut::ok(())
                                        })
                                }).spawn(ctx);
                        }

                        fut::ok(())
                    })
           
                     .spawn(ctx);
                      fut::ok(())
            }).spawn(ctx);

                return ();
            }
         
    
}

pub struct GetNode(pub String);

impl Message for GetNode {
    type Result = Result<Vec<NodeId>, ()>;
}

impl Handler<GetNode> for Network {
    type Result = Response<Vec<NodeId>, ()>;

    fn handle(&mut self, msg: GetNode, ctx: &mut Context<Self>) -> Self::Result {
        let raft = self.raft.as_mut().unwrap();
        Response::fut(raft.get_node(msg.0.as_str())
                      .map_err(|_| ())
                      .map(|res| res.unwrap())
        )
    }
}

pub struct Find(pub Vec<u8>);

impl Message for Find {
    type Result = Result<Vec<u8>, failure::Error>;
}

impl Handler<Find> for Network {
    type Result = Response<Vec<u8>, failure::Error>;

    fn handle(&mut self, msg: Find, ctx: &mut Context<Self>) -> Self::Result {
        let raft = self.raft.as_mut().unwrap();
        Response::fut(raft.find_value(msg.0)
                      .map_err(|_| failure::format_err!("{}","not find"))
                      .and_then(|res| res)
        )
    }
}


pub struct Save(pub String,pub String);

impl Message for Save {
    type Result = Result<(), ()>;
}

impl Handler<Save> for Network {
    type Result = Result<(),()>;

    fn handle(&mut self, msg: Save, ctx: &mut Context<Self>) -> Self::Result {
        let cr=ClientRequest(storage_add_data(msg.0,msg.1));
         ctx.address().do_send(cr);
        println!("have send to handle  leader or sendto network-----------------");

       
        Ok(())
    }
}



#[derive(Message)]
pub struct PeerConnected(pub NodeId);

impl Handler<PeerConnected> for Network {
    type Result = ();

    fn handle(&mut self, msg: PeerConnected, ctx: &mut Context<Self>) {
        // println!("Registering node {}", msg.0);
        self.nodes_connected.push(msg.0);

      
         
        // self.sessions.insert(msg.0, msg.1);
    }
}

#[derive(Message)]
pub struct InitRaft {
     pub nodes: Vec<NodeId>,
    
    pub net: Addr<Network>,

    pub join_mode: bool,
   
}

impl Handler<InitRaft> for Network {
    type Result = ();

    fn handle(&mut self, msg: InitRaft, ctx: &mut Context<Self>) {
               // let raft = self.raft.as_ref().unwrap();
        println!("members is ------{:?}",msg.nodes.clone());

                let network_addr = msg.net;
             
                 let nodes = msg.nodes;
                 let nodes_count=nodes.len();
   
       println!("the join mode is -------------{},the nodes len is {}",msg.join_mode,nodes_count);

        let nodes = if msg.join_mode && nodes_count==1 {
            vec![self.id]
        } else {
            nodes.clone()
        };

        println!("init raft members is ------{:?}",nodes.clone());

       let raft_node = RaftNode::new(self.id, nodes.clone(), network_addr);
        self.raft = Some(raft_node);

        if msg.join_mode && nodes_count==1 {
           
            return ();
        }

        if msg.join_mode && nodes_count>1 {

            /*
            fut::wrap_future::<_, Self>(
                self.raft.as_ref().unwrap().addr
                  .send(InitWithConfig::new(nodes.clone())),
          )
              .map_err(|err, _, _| panic!(err))
              .and_then(|_, act, ctx| {
                  println!("-----Inited with config-true->1-!");
                  println!("-----Inited with config-true->1-!");
                
            //      let payload = storage_add_node(act.id);
            //      ctx.notify(ClientRequest(payload));
                           
                   fut::ok(())
              }) .spawn(ctx);
              */
            
            /*
              fut::wrap_future::<_, Self>(Delay::new(Instant::now() + Duration::from_secs(5)))
              .map_err(|_, _, _| ())
              .and_then(move |_, act, ctx| {
                fut::ok(())
              }).spawn(ctx);
              */

        }

        if !msg.join_mode {

            fut::wrap_future::<_, Self>(Delay::new(Instant::now() + Duration::from_secs(5)))
            .map_err(|_, _, _| ())
            .and_then(move |_, act, ctx| {
                fut::wrap_future::<_, Self>(
                      act.raft.as_ref().unwrap().addr
                        .send(InitWithConfig::new(nodes.clone())),
                )
                    .map_err(|err, _, _| panic!(err))
                    .and_then(|_, _, _| {
                        println!("-----Inited with config---!");
                        println!("-----Inited with config---!");
                        println!("-----Inited with config---!");

                        fut::wrap_future::<_, Self>(Delay::new(
                            Instant::now() + Duration::from_secs(3),
                        ))
                    })
                    .map_err(|_, _, _| ())
                    .and_then(|_, act, ctx| {
                      let payload = storage_add_node(act.id);
                       ctx.notify(ClientRequest(payload));
                                
                        fut::ok(())
                    })
            })
            .spawn(ctx);
        }


            
  }
}

pub struct GetCurrentLeader;

impl Message for GetCurrentLeader {
    type Result = Result<NodeId, ()>;
}


impl Handler<GetCurrentLeader> for Network {
    type Result = ResponseActFuture<Self, NodeId, ()>;

    fn handle(&mut self, msg: GetCurrentLeader, ctx: &mut Context<Self>) -> Self::Result {
        if let Some(ref mut metrics) = self.metrics {
            if let Some(leader) = metrics.current_leader {
                Box::new(fut::result(Ok(leader)))
            } else {
                Box::new(
                    fut::wrap_future::<_, Self>(Delay::new(Instant::now() + Duration::from_secs(1)))
                        .map_err(|_, _, _| ())
                        .and_then(|_, _, ctx| {
                            fut::wrap_future::<_, Self>(ctx.address().send(msg))
                                .map_err(|_, _, _| ())
                                .and_then(|res, _, _| fut::result(res))
                        })
                )
            }
        } else {
            Box::new(
                    fut::wrap_future::<_, Self>(Delay::new(Instant::now() + Duration::from_secs(1)))
                        .map_err(|_, _, _| ())
                        .and_then(|_, _, ctx| {
                            fut::wrap_future::<_, Self>(ctx.address().send(msg))
                                .map_err(|_, _, _| ())
                                .and_then(|res, _, _| fut::result(res))
                        })
                )
        }
    }
}

pub struct ClientRequest(MemoryStorageData);

impl Message for ClientRequest {
    type Result = ();
}

impl Handler<ClientRequest> for Network {
    type Result = ();

    fn handle(&mut self, msg: ClientRequest, ctx: &mut Context<Self>) {

        let entry = messages::EntryNormal{data: msg.0.clone()};
        let payload = Payload::new(entry, messages::ResponseMode::Applied);

        let req = fut::wrap_future(ctx.address().send(GetCurrentLeader))
            .map_err(|err, _: &mut Network, _| ())
            .and_then(move |res, act, ctx| {
                let leader = res.unwrap();
                println!("Found leader: {}", leader);
                // if leader is current node send message here
                if act.id == leader {
                    println!("is leader and handle----------------");
                    let raft = act.raft.as_ref().unwrap();
                    fut::Either::A(fut::wrap_future::<_, Self>(raft.addr.send(payload))
                        .map_err(|_, _, _| ())
                        .and_then(move |res, act, ctx| {
                            match res {
                                Ok(_) => {
                                    fut::ok(())
                                },
                                Err(err) => match err {
                                    messages::ClientError::Internal => {
                                        debug!("TEST: resending client request.");
                                        ctx.notify(msg);
                                        fut::ok(())
                                    }
                                    messages::ClientError::Application(err) => {
                                        panic!("Unexpected application error from client request: {:?}", err);
                                    }
                                    messages::ClientError::ForwardToLeader{leader, ..} => {
                                        debug!("TEST: received ForwardToLeader error. Updating leader and forwarding.");
                                        ctx.notify(msg);
                                        fut::ok(())
                                    }
                                }
                            }
                        })
                    )

                } else {
                    // send to remote raft

                    println!("not a leader  send to remote raft--------");
                    let node = act.get_node(leader).unwrap();
                    fut::Either::B(fut::wrap_future::<_, Self>(node.send(SendRaftMessage(payload)))
                        .map_err(|_, _, _| ())
                        .and_then(move |res, act, ctx| {
                            match res {
                                Ok(_) => {
                                    fut::ok(())
                                },
                                Err(err) => match err {
                                    messages::ClientError::Internal => {
                                        debug!("TEST: resending client request.");
                                        ctx.notify(msg);
                                        fut::ok(())
                                    }
                                    messages::ClientError::Application(err) => {
                                        panic!("Unexpected application error from client request: {:?}", err)
                                    }
                                    messages::ClientError::ForwardToLeader{leader, ..} => {
                                        debug!("TEST: received ForwardToLeader error. Updating leader and forwarding.");
                                        ctx.notify(msg);
                                        fut::ok(())
                                    }
                                }
                            }
                        })
                    )
                }
            });

        ctx.spawn(req);
    }
}



pub struct GetNodes;

impl Message for GetNodes {
    type Result = Result<HashMap<NodeId, String>, ()>;
}

impl Handler<GetNodes> for Network {
    type Result = Result<HashMap<NodeId, String>, ()>;

    fn handle(&mut self, _: GetNodes, ctx: &mut Context<Self>) -> Self::Result {

        //add myself node 
        self.all_connected_nodes.insert(self.id,self.address.as_ref().unwrap().clone());
        Ok(self.all_connected_nodes.clone())
    }
}



pub struct AddConnectedNode(pub NodeId,pub String);

impl Message for AddConnectedNode {
    type Result = ();
}

impl Handler<AddConnectedNode> for Network {
    type Result = ();

    fn handle(&mut self, msg: AddConnectedNode, ctx: &mut Context<Self>) -> Self::Result {        
        //add local node 
        self.all_connected_nodes.insert(msg.0,msg.1.clone());
       
    }
}

/*
pub struct RemoveConnectedNode(pub NodeId);

impl Message for RemoveConnectedNode {
    type Result = ();
}

impl Handler<RemoveConnectedNode> for Network {
    type Result = ();

    fn handle(&mut self, msg: RemoveConnectedNode, ctx: &mut Context<Self>) -> Self::Result {        
        //add local node 
        self.all_connected_nodes.remove(&msg.0);
       
    }
}
*/

//////////////////////////////////////////////////////////////////////////////
// RaftMetrics ///////////////////////////////////////////////////////////////

impl Handler<RaftMetrics> for Network {
    type Result = ();

    fn handle(&mut self, msg: RaftMetrics, _: &mut Context<Self>) -> Self::Result {
        println!("Metrics: node={} state={:?} leader={:?} term={} index={} applied={} cfg={{join={} members={:?} non_voters={:?} removing={:?}}}",
                 msg.id, msg.state, msg.current_leader, msg.current_term, msg.last_log_index, msg.last_applied,
                 msg.membership_config.is_in_joint_consensus, msg.membership_config.members,
                 msg.membership_config.non_voters, msg.membership_config.removing,
        );
        self.metrics = Some(msg);
    }
}


pub struct DiscoverNodes;

impl Message for DiscoverNodes {
    type Result = Result<(Vec<NodeId>, bool), ()>;
}

impl Handler<DiscoverNodes> for Network {
    type Result = ResponseActFuture<Self, (Vec<NodeId>, bool), ()>;

    fn handle(&mut self, _: DiscoverNodes, _: &mut Context<Self>) -> Self::Result {
        Box::new(
            fut::wrap_future::<_, Self>(Delay::new(Instant::now() + Duration::from_secs(8)))
                .map_err(|_, _, _| ())
                .and_then(|_, act: &mut Network, _| fut::result(Ok((act.nodes_connected.clone(), act.join_mode)))),
        )
    }
}


#[derive(Message)]
pub struct RemoveNode(pub NodeId);

impl Handler<RemoveNode> for Network {
    type Result = ();

    fn handle(&mut self, msg: RemoveNode, ctx: &mut Context<Self>) {
       println!("RemoveNode   delete-----------------{}",msg.0);
    
       let fu= ctx.address().send(ChangeRaftClusterConfig(vec![], vec![msg.0]))
        .map_err(|err| println!("this is ---------------{:?}",err))
        .map(|_| ());

         Arbiter::spawn(fu);
      

    }
}




#[derive(Serialize, Deserialize ,Message, Clone)]
pub struct ChangeRaftClusterConfig(pub Vec<NodeId>, pub Vec<NodeId>);

impl Handler<ChangeRaftClusterConfig> for Network {
    type Result = ();

    fn handle(&mut self, msg: ChangeRaftClusterConfig, ctx: &mut Context<Self>) {
        let nodes_to_add = msg.0.clone();
        let nodes_to_remove = msg.1.clone();

        use actix_raft::admin::ProposeConfigChangeError;

        let payload = ProposeConfigChange::new(nodes_to_add.clone(), nodes_to_remove.clone());

        ctx.spawn(
            fut::wrap_future::<_, Self>(ctx.address().send(GetCurrentLeader))
                .map_err(|err, _, _| panic!(err))
                .and_then(move |res, act, ctx| {
                    let leader = res.unwrap();

                    if leader == act.id {
                        if let Some(ref raft) = act.raft {
                            println!(" ------------- About to propose config change");
                            return fut::Either::A(
                                fut::wrap_future::<_, Self>(raft.addr.send(payload))
                                    .map_err(|err, _, _| {
                                        println!("About to propose config change error is {:?}",err);
                                      //  panic!(err)
                                        })
                                    .and_then(move |res, act, ctx| {
                                        println!("-----------propose config change-----{:?}-----",res);
                                        let flag=match res {
                                            Ok(_) => true,
                                            Err(ProposeConfigChangeError::Noop) => true,
                                            _ => false,
                                        };
                                     if flag {
                                     println!("--1---------succesful to propose config change-----{:?}-----",res);

                                        for id in nodes_to_add.iter() {
                                         
                                      let payload = storage_add_node(*id);
                                       ctx.notify(ClientRequest(payload));
                                        }

                                        for id in nodes_to_remove.iter() {
                                     let payload= storage_remove_node(*id);
                                   
                                      ctx.notify(ClientRequest(payload));
                                        }

                                         
                                        println!("---------succesful to propose config change------");

                                        fut::ok(())
                                     } else {
                                        println!("---------failure to propose config change------");
                                         fut::err(())
                                     }
                                    }),
                            );
                        }
                    }

                    fut::Either::B(
                        fut::wrap_future::<_, Self>(ctx.address().send(GetNodeById(leader)))
                            .map_err(move |err, _, _| panic!("Node {} not found", leader))
                            .and_then(move |node, act, ctx| {
                                println!("-------------- Sending remote proposal to leader");

                                let json_string=serde_json::ser::to_string(&msg.clone()).unwrap();
                                
                                let send_to_raft_po=SendToRaft(MsgTypes::ChangeRaftClusterConfig,json_string);

                                fut::wrap_future(
                                     node.unwrap().send(send_to_raft_po)
                                 )
                                    .map_err(|err, _, _| println!("send remote proposal Error {:?}", err))
                                    .and_then(|res, act, ctx| {
                                        fut::ok(())
                                    })
                                    
                                  
                            }),
                    )
                }),
        );
    }
}



pub struct GetNodeById(pub NodeId);

impl Message for GetNodeById {
    type Result = Result<Addr<Node>, ()>;
}

impl Handler<GetNodeById> for Network {
    type Result = Result<Addr<Node>, ()>;

    fn handle(&mut self, msg: GetNodeById, ctx: &mut Context<Self>) -> Self::Result {
        if let Some(ref node) = self.get_node(msg.0) {
            Ok((*node).clone())
        } else {
            Err(())
        }
    }
}


#[derive(Message,Debug)]
pub struct Handshake(pub NodeId, pub String);

impl Handler<Handshake> for Network {
    type Result = ();

    fn handle(&mut self, msg: Handshake, ctx: &mut Context<Self>) {
        println!("in handshke -----{:?}",msg);
        self.all_connected_nodes.insert(msg.0, msg.1.clone());
        self.register_node(&msg.1, ctx.address().clone());
    }
}

#[derive(Message)]
pub struct NodeDisconnect(pub NodeId);

impl Handler<NodeDisconnect> for Network {
    type Result = ();

    fn handle(&mut self, msg: NodeDisconnect, ctx: &mut Context<Self>) {
        let id = msg.0;
  
        self.nodes_connected.retain(|item| item!=&id);
   //     println!("the crrent nodes_connected is {:?}-----",self.nodes_connected);
        self.all_connected_nodes.remove(&id);
    
        self.nodes.remove(&id);
        self.isolated_nodes.push(id);
 
      
        // RemoveNode only if node is leader
        fut::wrap_future::<_, Self>(ctx.address().send(GetCurrentLeader))
            .map_err(|_, _, _| ())
            .and_then(move |res, act, ctx| {
                let leader = res.unwrap();
            
                if leader == id {
                    fut::wrap_future::<_, Self>(Delay::new(Instant::now() + Duration::from_secs(1)))
                        .map_err(|_, _, _| ())
                        .and_then(|_, act, ctx| {
                            ctx.notify(msg);
                            fut::ok(())
                        }).spawn(ctx);

                    return fut::ok(());
                }

                if leader == act.id {
                    /*     
                      Arbiter::spawn(ctx.address().send(RemoveNode(id))
                                   .map_err(|_| ())
                                   .and_then(|res| {
                                        println!("NodeDisconnect leader == act.id................................");
                                       futures::future::ok(())
                                   }));
                                   */
                     let s=   fut::wrap_future(ctx.address().send(RemoveNode(id)))
                        .map_err(|_, _: &mut Self,_| ())
                        .and_then(|_res,_,_| {
                                        println!("NodeDisconnect leader == act.id................................");
                                       fut::ok(())
                                   });
                     s.spawn(ctx);              
                }
                
                fut::ok(())
          
            }).spawn(ctx);
    }
}




pub struct GetListenAddress;

impl Message for GetListenAddress {
    type Result = Result<String, ()>;
}

impl Handler<GetListenAddress> for Network {
    type Result =  Result<String, ()>;

    fn handle(&mut self, _msg: GetListenAddress, ctx: &mut Context<Self>) -> Self::Result {
        let network_address = self.address.as_ref().unwrap().clone();
        println!("in GetListenAddress get ---{}",network_address);
        Ok(network_address)
        
    }
}

#[derive(Message)]
pub struct RestoreNode(pub NodeId);

impl Handler<RestoreNode> for Network {
    type Result = ();

    fn handle(&mut self, msg: RestoreNode, ctx: &mut Context<Self>) {
        let id = msg.0;
        self.restore_node(id);
    }
}


pub struct GetClusterState;

impl Message for GetClusterState {
    type Result = Result<NetworkState, ()>;
}

impl Handler<GetClusterState> for Network {
    type Result = Result<NetworkState, ()>;

    fn handle(&mut self, _: GetClusterState, ctx: &mut Context<Self>) -> Self::Result {
        Ok(self.state.clone())
    }
}

#[derive(Message)]
pub struct SetClusterState(pub NetworkState);

impl Handler<SetClusterState> for Network {
    type Result = ();

    fn handle(&mut self, msg: SetClusterState, ctx: &mut Context<Self>) {
        self.state = msg.0;
    }
}

pub struct SendToRaft(pub MsgTypes, pub String);

impl Message for SendToRaft {
    type Result = Result<String, ()>;
}

impl Handler<SendToRaft> for Network {
    type Result = Response<String, ()>;

    fn handle(&mut self, msg: SendToRaft, _ctx: &mut Context<Self>) -> Self::Result {
        let type_id = msg.0;
        let body = msg.1;

        let res = if let Some(ref mut raft) = self.raft {
            match type_id {
                MsgTypes::AppendEntriesRequest => {
                    let raft_msg = serde_json::from_slice::<
                        messages::AppendEntriesRequest<storage::MemoryStorageData>,
                    >(body.as_ref())
                    .unwrap();

                    let future = raft.addr.send(raft_msg).map_err(|_| ()).and_then(|res| {
                        let res_payload = serde_json::to_string(&res).unwrap();
                        futures::future::ok(res_payload)
                    });
                    Response::fut(future)
                }
                MsgTypes::VoteRequest => {
                    let raft_msg =
                        serde_json::from_slice::<messages::VoteRequest>(body.as_ref()).unwrap();

                    let future = raft.addr.send(raft_msg).map_err(|_| ()).and_then(|res| {
                        let res_payload = serde_json::to_string(&res).unwrap();
                        futures::future::ok(res_payload)
                    });
                    Response::fut(future)
                }
                MsgTypes::InstallSnapshotRequest => {
                    let raft_msg =
                        serde_json::from_slice::<messages::InstallSnapshotRequest>(body.as_ref())
                            .unwrap();

                    let future = raft.addr.send(raft_msg).map_err(|_| ()).and_then(|res| {
                        let res_payload = serde_json::to_string(&res).unwrap();
                        futures::future::ok(res_payload)
                    });
                    Response::fut(future)
                },
                MsgTypes::ClientPayload => {
                    let raft_msg =
                        serde_json::from_slice::<messages::ClientPayload<storage::MemoryStorageData, storage::MemoryStorageResponse, storage::MemoryStorageError>>(body.as_ref())
                            .unwrap();

                    let future = raft.addr.send(raft_msg).map_err(|_| ()).and_then(|res| {
                        let res_payload = serde_json::to_string(&res).unwrap();
                        futures::future::ok(res_payload)
                    });
                    Response::fut(future)
                },
                  MsgTypes::ChangeRaftClusterConfig => {
                    let msg =
                        serde_json::from_slice::<ChangeRaftClusterConfig>(body.as_ref())
                            .unwrap();

                let nodes_to_add = msg.0.clone();
                let nodes_to_remove = msg.1.clone();

                let payload = ProposeConfigChange::new(nodes_to_add.clone(), nodes_to_remove.clone());
                    

                    let future = raft.addr.send(payload).map_err(|_| ()).and_then(|res| {
                        println!("get raft sendto message propal is resutl -----{:?}",res);
                        let res_payload = serde_json::to_string("propal ").unwrap();
                        futures::future::ok(res_payload)
                    });
                    Response::fut(future)
                },
                _ => Response::reply(Ok("".to_owned())),
            }
        } else {
            Response::reply(Ok("".to_owned()))
        };

        res
    }
}


fn storage_add_node(id: NodeId) -> MemoryStorageData {
    MemoryStorageData::Add(id)
}

fn storage_remove_node(id: NodeId) -> MemoryStorageData {
    MemoryStorageData::Remove(id)
}

fn storage_add_data(key: String,value:String) -> MemoryStorageData {
    MemoryStorageData::My(key,value)
}








