use actix::prelude::*;
use actix_raft::{RaftNetwork, messages};
use log::{error};

use crate::network::{
    Network,
    remote::{SendRaftMessage},
};
use crate::raft::{
    storage::{
        MemoryStorageData as Data
    }
};


const ERR_ROUTING_FAILURE: &str = "Failed to send RCP to node target.";

impl RaftNetwork<Data> for Network {}

impl Handler<messages::AppendEntriesRequest<Data>> for Network {
    type Result = ResponseActFuture<Self, messages::AppendEntriesResponse, ()>;

    fn handle(&mut self, msg: messages::AppendEntriesRequest<Data>, _ctx: &mut Context<Self>) -> Self::Result {
       // println!("the node id is ----{}",msg.target);
       //   println!("the AppendEntriesRequest  is ----{:?}",msg);
        match self.get_node(msg.target) {
            Some(node) => {

           if self.isolated_nodes.contains(&msg.target)  || self.isolated_nodes.contains(&msg.leader_id) {
                return Box::new(fut::err(()));
            }
              let req = node.send(SendRaftMessage(msg));

               Box::new(fut::wrap_future(req)
            .map_err(|_, _, _| error!("{}", ERR_ROUTING_FAILURE))
            .and_then(|res, _, _| fut::result(res)))
            },
           None => {
                  Box::new(fut::err(()))
            }
        }
        
      
    }
}

impl Handler<messages::VoteRequest> for Network {
    type Result = ResponseActFuture<Self, messages::VoteResponse, ()>;

    fn handle(&mut self, msg: messages::VoteRequest, _ctx: &mut Context<Self>) -> Self::Result {
  // println!("the VoteRequest  is ----{:?}",msg);
      match self.get_node(msg.target) {
            Some(node) => {
                  if self.isolated_nodes.contains(&msg.target) || self.isolated_nodes.contains(&msg.candidate_id) {
                return Box::new(fut::err(()));
            }
              let req = node.send(SendRaftMessage(msg));

               Box::new(fut::wrap_future(req)
            .map_err(|_, _, _| error!("{}", ERR_ROUTING_FAILURE))
            .and_then(|res, _, _| fut::result(res)))
            },
           None => {
                  Box::new(fut::err(()))
            }
        }
        
      
    }
}

impl Handler<messages::InstallSnapshotRequest> for Network {
    type Result = ResponseActFuture<Self, messages::InstallSnapshotResponse, ()>;

    fn handle(&mut self, msg: messages::InstallSnapshotRequest, _ctx: &mut Context<Self>) -> Self::Result {
         match self.get_node(msg.target) {
            Some(node) => {
                  if self.isolated_nodes.contains(&msg.target) || self.isolated_nodes.contains(&msg.leader_id) {
                return Box::new(fut::err(()));
            }
              let req = node.send(SendRaftMessage(msg));

               Box::new(fut::wrap_future(req)
            .map_err(|_, _, _| error!("{}", ERR_ROUTING_FAILURE))
            .and_then(|res, _, _| fut::result(res)))
            },
           None => {
                  Box::new(fut::err(()))
            }
        }
        
      
    }
    
}
