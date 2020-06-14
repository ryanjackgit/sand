use std::{
    collections::BTreeMap,
    io::{Seek, SeekFrom, Write},
    fs::{self, File},
    path::PathBuf,
};

use actix::prelude::*;
use log::{debug, error};
use serde::{Serialize, Deserialize};
use rmp_serde as rmps;
use hash_ring::HashRing;
use std::collections::HashSet;

use failure::Error;

use rocksdb::DB;
use std::sync::Arc;


use crate::rocksstore::db::{insert,get,delete};

use actix_raft::{
    AppData, AppDataResponse, AppError, NodeId,
    messages::{Entry as RaftEntry, EntryPayload, EntrySnapshotPointer, MembershipConfig},
    storage::{
        AppendEntryToLog,
        ReplicateToLog,
        ApplyEntryToStateMachine,
        ReplicateToStateMachine,
        CreateSnapshot,
        CurrentSnapshotData,
        GetCurrentSnapshot,
        GetInitialState,
        GetLogEntries,
        HardState,
        InitialState,
        InstallSnapshot,
        RaftStorage,
        SaveHardState,
    },
};



#[derive(Clone, Debug, Serialize, Deserialize)]
 pub struct  InitialStateData  {
    last_log_index:u64,
    last_log_term:u64,
    last_applied_log:u64,
    hard_state:HardState,

 }

type Entry = RaftEntry<MemoryStorageData>;

/// The concrete data type used by the `MemoryStorage` system.
/*
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MemoryStorageData(pub NodeId);
*/

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MemoryStorageData {
    Add(NodeId),
    Remove(NodeId),
    My(String,String),
}


impl AppData for MemoryStorageData {}

/// The concrete data type used for responding from the storage engine when applying logs to the state machine.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MemoryStorageResponse;



impl AppDataResponse for MemoryStorageResponse {}

/// The concrete error type used by the `MemoryStorage` system.
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryStorageError;

impl std::fmt::Display for MemoryStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: give this something a bit more meaningful.
        write!(f, "MemoryStorageError")
    }
}

impl std::error::Error for MemoryStorageError {}

impl AppError for MemoryStorageError {}

/// A concrete implementation of the `RaftStorage` trait.
///
/// This is primarity for testing and demo purposes. In a real application, storing Raft's data
/// on a stable storage medium is expected.
///
/// This storage implementation structures its data as an append-only immutable log. The contents
/// of the entries given to this storage implementation are not ready or manipulated.
pub struct MemoryStorage {
    hs: HardState,
 //   log: BTreeMap<u64, Entry>,
    snapshot_data: Option<CurrentSnapshotData>,
    snapshot_dir: String,
 //   state_machine: BTreeMap<u64, Entry>,
    snapshot_actor: Addr<SnapshotActor>,
  
    add_remove_nodes:HashSet<NodeId>,
    db:Arc<DB>,
}

impl MemoryStorage {
    /// Create a new instance.
    pub fn new(members: Vec<NodeId>, snapshot_dir: String) -> Self {
        let db=crate::rocksstore::db::open_db("/tmp/rocksdata");
        let snapshot_dir_pathbuf = std::path::PathBuf::from(snapshot_dir.clone());
        let membership = MembershipConfig{members, non_voters: vec![], removing: vec![], is_in_joint_consensus: false};
        Self{
            hs: HardState{current_term: 0, voted_for: None, membership},
           // log: Default::default(),
            snapshot_data: None, snapshot_dir,
            db:Arc::new(db),
         ///   state_machine: Default::default(),
            snapshot_actor: SyncArbiter::start(1, move || SnapshotActor(snapshot_dir_pathbuf.clone())),
          
            add_remove_nodes:HashSet::new(),
        }
    }

    // don't include end
    fn get_logs_entry(&self,begin:u64,end:u64) -> Vec<Entry>   {
        println!(" range is ---------------------{} to {}",begin,end);
        let mut entrys=Vec::new();

      
     //   use rocksdb::SliceTransform;
      //  let prefix_extractor = SliceTransform::create("first_three", first_three, None);

        let mut log_iterator = self.db.prefix_iterator(b"000");
    //  let mut log_iterator = self.db.prefix_iterator(prefix_extractor);
        let index=format!("{}{}","000",begin);

      
  
        let (mut vecone,_):(Vec<(std::boxed::Box<[u8]>,std::boxed::Box<[u8]>)>,Vec<(std::boxed::Box<[u8]>,std::boxed::Box<[u8]>)>)=log_iterator.partition(|(key,value)| {

            let mut key=String::from_utf8(key.to_vec()).unwrap();
            key.drain(0..3).collect::<String>();
            let key=key.parse::<u64>().unwrap_or(0);      
            key>=begin
        });

        use std::cmp::Ordering;
        fn log_sequence_comparator(one:&(std::boxed::Box<[u8]>,std::boxed::Box<[u8]>),two:&(std::boxed::Box<[u8]>,std::boxed::Box<[u8]>)) -> Ordering {
        
              let one_string=String::from_utf8(one.0[3..].to_vec()).unwrap_or("0".to_string());
              let two_string=String::from_utf8(two.0[3..].to_vec()).unwrap_or("0".to_string());
              let one_size:u64=one_string.parse().unwrap_or(0);
              let two_size:u64=two_string.parse().unwrap_or(0);
              one_size.cmp(&two_size)
        
             }

        vecone.sort_by(log_sequence_comparator);

        let mut it=vecone.into_iter();
        while let Some(value) =it.next() {
         
          let  key=String::from_utf8((&value.0).to_vec()).unwrap();
          let endone=format!("{}{}","000",end);
          println!("get logs key and end is ---------------------{} to {}",key,endone);
          if key==endone {
           // println!("get logs key and end is ---------------------{} to {}",key,endone);
              break;
          }

          let en:Entry=rmps::from_read_ref(&value.1).unwrap();
      
          entrys.push(en);
          
       }

       entrys

    }
}

impl Actor for MemoryStorage {
    type Context = Context<Self>;

  
    fn started(&mut self, _ctx: &mut Self::Context) {}
}

impl RaftStorage<MemoryStorageData, MemoryStorageResponse, MemoryStorageError> for MemoryStorage {
    type Actor = Self;
    type Context = Context<Self>;
}

impl Handler<GetInitialState<MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, InitialState, MemoryStorageError>;

    fn handle(&mut self, msg: GetInitialState<MemoryStorageError>, _: &mut Self::Context) -> Self::Result {
           println!("------------GetInitialState---------------- ");
           let log_iterator = self.db.prefix_iterator(b"000");
           let (index,term)=log_iterator.last().map(|(key,value)| {
            //  key.drain(0..3).collect::<String>();
           //   let index=key.parse::<u64>().unwrap_or(0);
            let log_item:actix_raft::messages::Entry<MemoryStorageData>= rmps::from_read_ref(&value).unwrap();
          
            let index=log_item.index;
            let term=log_item.term;
            (index,term)
           }).unwrap_or((0,0));

           let data_iterator = self.db.prefix_iterator(b"001");
           let (key,applied_log)=data_iterator.last().map(|(key,value)| {
                 let mut key=String::from_utf8((&key).to_vec()).unwrap();
                   key.drain(0..3).collect::<String>();
               let key=key.parse::<u64>().unwrap_or(0);
            let apply_item:actix_raft::messages::Entry<MemoryStorageData>= rmps::from_read_ref(&value).unwrap();
          
            (key,apply_item.index)
           }).unwrap_or((0,0));

           let hs_iterator = self.db.prefix_iterator(b"002");
           let hs=hs_iterator.last().map(|(_key,value)| {
            let hs:HardState=rmps::from_read_ref(&value).unwrap();
             hs
           }).unwrap_or(self.hs.clone());

         println!("-----the initial hardstate-----{:?}",hs.clone());
         println!("-----log index:{} term:{} ,key:{},applied_log:{}",index,term,key,applied_log);

        let init = InitialState{
            last_log_index:index ,
            last_log_term: term,
            last_applied_log: applied_log,
            hard_state: hs.clone(),
        };

        self.hs=hs;

        Box::new(fut::ok(init))
    }
}

impl Handler<SaveHardState<MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, (), MemoryStorageError>;

    fn handle(&mut self, msg: SaveHardState<MemoryStorageError>, _: &mut Self::Context) -> Self::Result {
        println!("------------save the SaveHardState ---------------- {:?}",msg.hs);
/*
        let hs_iterator = self.db.prefix_iterator(b"002");

        let hs=hs_iterator.last().map(|(_key,value)| {
         let hs:HardState=rmps::from_read_ref(&value).unwrap();
          hs
        }).unwrap_or(msg.hs.clone());

        if hs==msg.hs {
            self.hs = msg.hs.clone();
        } else {
        let buf = rmps::to_vec(&msg.hs).unwrap();
        insert(self.db.clone(),b"002hs",&buf);
        self.hs=hs;
        }
*/
      
       let buf = rmps::to_vec(&msg.hs).unwrap();
       insert(self.db.clone(),b"002hs",&buf);
        self.hs=msg.hs.clone();
        Box::new(fut::ok(()))
    }
}

impl Handler<GetLogEntries<MemoryStorageData, MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, Vec<Entry>, MemoryStorageError>;

    fn handle(&mut self, msg: GetLogEntries<MemoryStorageData, MemoryStorageError>, _: &mut Self::Context) -> Self::Result {
      //  println!("----GetLogEntries----------------------------------");
        let entrys=self.get_logs_entry(msg.start,msg.stop);
        Box::new(fut::ok(entrys))
    }
}



impl Handler<AppendEntryToLog<MemoryStorageData, MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, (), MemoryStorageError>;

    fn handle(&mut self, msg: AppendEntryToLog<MemoryStorageData, MemoryStorageError>, _: &mut Self::Context) -> Self::Result {
          println!("---AppendEntryToLog-----------main logic -----------------------");
       // self.log.insert(msg.entry.index, (*msg.entry).clone());
       let buf = rmps::to_vec(&(*msg.entry).clone()).unwrap();
       let log_prefix=format!("{}{}","000",msg.entry.index);
        insert(self.db.clone(),log_prefix.as_bytes(),&buf);
        Box::new(fut::ok(()))
    }
}

impl Handler<ReplicateToLog<MemoryStorageData, MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, (), MemoryStorageError>;

    fn handle(&mut self, msg: ReplicateToLog<MemoryStorageData, MemoryStorageError>, _: &mut Self::Context) -> Self::Result {
          println!("---ReplicateToLog----------------------------------");
        msg.entries.iter().for_each(|e| {
            //self.log.insert(e.index, e.clone());
            let buf = rmps::to_vec(&e.clone()).unwrap();
            let log_prefix=format!("{}{}","000",e.index);
            insert(self.db.clone(),log_prefix.as_bytes(),&buf);
        });
        Box::new(fut::ok(()))
    }
}

impl Handler<ApplyEntryToStateMachine<MemoryStorageData, MemoryStorageResponse, MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, MemoryStorageResponse, MemoryStorageError>;

    fn handle(&mut self, msg: ApplyEntryToStateMachine<MemoryStorageData, MemoryStorageResponse, MemoryStorageError>, _ctx: &mut Self::Context) -> Self::Result {
            println!("---ApplyEntryToStateMachine----------------------------------");

            let buf = rmps::to_vec(&(*msg.payload).clone()).unwrap();
            let log_prefix=format!("{}{}","001",msg.payload.index);
        

        let res = if let Err(_) = insert(self.db.clone(),log_prefix.as_bytes(),&buf) {
            println!(" error. State machine. Entry: {:?}", (*msg.payload).clone());
            Err(MemoryStorageError)
        } else {

           if let EntryPayload::Normal(entry) = &msg.payload.payload {
              
                match (*entry).data {
                    MemoryStorageData::Add(node_id) => {
                        println!("ApplyEntryToStateMachine Adding node {}", node_id);
                        self.add_remove_nodes.insert(node_id);
                      
                    }
                    MemoryStorageData::Remove(node_id) => {
                        println!("ApplyEntryToStateMachine Removing node {}", node_id);
                        self.add_remove_nodes.remove(&node_id);
                    }
                      MemoryStorageData::My(ref key,ref value) => {
                     
                     // self.mydata_nodes.add_node(&node_id);
                     let buf = rmps::to_vec(&value.clone()).unwrap();
                     let log_prefix=format!("{}{}","003",key);
                     insert(self.db.clone(),log_prefix.as_bytes(),&buf);
                    }
                }
            }  else {

            }

            Ok(MemoryStorageResponse)
        };
        Box::new(fut::result(res))
    }
}

impl Handler<ReplicateToStateMachine<MemoryStorageData, MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, (), MemoryStorageError>;

    fn handle(&mut self, msg: ReplicateToStateMachine<MemoryStorageData, MemoryStorageError>, _ctx: &mut Self::Context) -> Self::Result {
          println!("---ReplicateToStateMachine----------------------------------");
        let res = msg.payload.iter().try_for_each(|e| {
            let buf = rmps::to_vec(&e.clone()).unwrap();
            let log_prefix=format!("{}{}","001",e.index);
          
            println!("the log_prefix is --------------------{:?}",log_prefix.clone());
            if let Err(_) =  insert(self.db.clone(),log_prefix.as_bytes(),&buf) {
                println!(" error. State machine entires insert error. Entry: {:?}", e.clone());
                return Err(MemoryStorageError)
            }
       
                  if let EntryPayload::Normal(entry) = &e.payload {
              
                match (*entry).data {
                    MemoryStorageData::Add(node_id) => {
                        println!("ReplicateToStateMachine Adding node {}", node_id);
                        self.add_remove_nodes.insert(node_id);
                      
                    }
                    MemoryStorageData::Remove(node_id) => {
                        println!("ReplicateToStateMachine Removing node {}", node_id);
                        self.add_remove_nodes.remove(&node_id);
                    }
                    MemoryStorageData::My(ref key,ref value) => {
                     
                        let buf = rmps::to_vec(&value.clone()).unwrap();
                        let log_prefix=format!("{}{}","003",key);
                        insert(self.db.clone(),log_prefix.as_bytes(),&buf);
                    }
                }
            } else {

            }

            Ok(())
        });
        Box::new(fut::result(res))
    }
}

impl Handler<CreateSnapshot<MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, CurrentSnapshotData, MemoryStorageError>;

    fn handle(&mut self, msg: CreateSnapshot<MemoryStorageError>, _: &mut Self::Context) -> Self::Result {
        println!("----------------Creating new snapshot under '{}' through index {}.-----------", &self.snapshot_dir, &msg.through);
        // Serialize snapshot data.
        let through = msg.through;
        
       // let entries = self.log.range(0u64..=through).map(|(_, v)| v.clone()).collect::<Vec<_>>();
        let entries=self.get_logs_entry(0,through+1);

        println!("Creating snapshot with {} entries.", entries.len());

        let (index, term) = entries.last().map(|e| (e.index, e.term)).unwrap_or((0, 0));
        let snapdata = match rmps::to_vec(&entries) {
            Ok(snapdata) => snapdata,
            Err(err) => {
                println!("Error serializing log for creating a snapshot. {}", err);
                return Box::new(fut::err(MemoryStorageError));
            }
        };

        // Create snapshot file and write snapshot data to it.
        let filename = format!("{}", msg.through);
        let filepath = std::path::PathBuf::from(self.snapshot_dir.clone()).join(filename);
        Box::new(fut::wrap_future(self.snapshot_actor.send(CreateSnapshotWithData(filepath.clone(), snapdata)))
            .map_err(|err, _, _| panic!("Error communicating with snapshot actor. {}", err))
            .and_then(|res, _, _| fut::result(res))
            // Clean up old log entries which are now part of the new snapshot.
            .and_then(move |_, act: &mut Self, _| {
                let path = filepath.to_string_lossy().to_string();
                debug!("Finished creating snapshot file at {}", &path);
         //       act.log = act.log.split_off(&through);
                let pointer = EntrySnapshotPointer{path};
                let entry = Entry::new_snapshot_pointer(pointer.clone(), index, term);
         //       act.log.insert(through, entry);

                // Cache the most recent snapshot data.
                let current_snap_data = CurrentSnapshotData{term, index, membership: act.hs.membership.clone(), pointer};
                act.snapshot_data = Some(current_snap_data.clone());

                fut::ok(current_snap_data)
            }))
    }
}

impl Handler<InstallSnapshot<MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, (), MemoryStorageError>;

    fn handle(&mut self, msg: InstallSnapshot<MemoryStorageError>, _: &mut Self::Context) -> Self::Result {
        println!("--------------------InstallSnapshot---------------------");
        let (index, term) = (msg.index, msg.term);
        Box::new(fut::wrap_future(self.snapshot_actor.send(SyncInstallSnapshot(msg)))
            .map_err(|err, _, _| panic!("Error communicating with snapshot actor. {}", err))
            .and_then(|res, _, _| fut::result(res))

            // Snapshot file has been created. Perform final steps of this algorithm.
            .and_then(move |pointer, act: &mut Self, ctx| {
                // Cache the most recent snapshot data.
                act.snapshot_data = Some(CurrentSnapshotData{index, term, membership: act.hs.membership.clone(), pointer: pointer.clone()});

                // Update target index with the new snapshot pointer.
                let entry = Entry::new_snapshot_pointer(pointer.clone(), index, term);
                /*
               act.log = act.log.split_off(&index);
                let previous = act.log.insert(index, entry);

                // If there are any logs newer than `index`, then we are done. Else, the state
                // machine should be reset, and recreated from the new snapshot.
                match &previous {
                    Some(entry) if entry.index == index && entry.term == term => {
                        fut::Either::A(fut::ok(()))
                    }
                    // There are no newer entries in the log, which means that we need to rebuild
                    // the state machine. Open the snapshot file read out its entries.
                    _ => {
                        let pathbuf = PathBuf::from(pointer.path);
                        fut::Either::B(act.rebuild_state_machine_from_snapshot(ctx, pathbuf))
                    }
                }
                */
                fut::ok(())
            }))
    }
}

impl Handler<GetCurrentSnapshot<MemoryStorageError>> for MemoryStorage {
    type Result = ResponseActFuture<Self, Option<CurrentSnapshotData>, MemoryStorageError>;

    fn handle(&mut self, _: GetCurrentSnapshot<MemoryStorageError>, _: &mut Self::Context) -> Self::Result {
        println!("-----------------Checking for current snapshot.---------------");
        Box::new(fut::ok(self.snapshot_data.clone()))
    }
}

impl MemoryStorage {
    /// Rebuild the state machine from the specified snapshot.
    fn rebuild_state_machine_from_snapshot(&mut self, _: &mut Context<Self>, path: std::path::PathBuf) -> impl ActorFuture<Actor=Self, Item=(), Error=MemoryStorageError> {
        // Read full contents of the snapshot file.
        fut::wrap_future(self.snapshot_actor.send(DeserializeSnapshot(path)))
            .map_err(|err, _, _| panic!("Error communicating with snapshot actor. {}", err))
            .and_then(|res, _, _| fut::result(res))
            // Rebuild state machine from the deserialized data.
            .and_then(|entries, act: &mut Self, _| {
               // act.state_machine.clear();
              //  act.state_machine.extend(entries.into_iter().map(|e| (e.index, e)));
                fut::ok(())
            })
            .map(|_, _, _| debug!("Finished rebuilding statemachine from snapshot successfully."))
    }
}

//////////////////////////////////////////////////////////////////////////////////////////////////
// SnapshotActor /////////////////////////////////////////////////////////////////////////////////

/// A simple synchronous actor for interfacing with the filesystem for snapshots.
struct SnapshotActor(std::path::PathBuf);

impl Actor for SnapshotActor {
    type Context = SyncContext<Self>;
}

struct SyncInstallSnapshot(InstallSnapshot<MemoryStorageError>);

//////////////////////////////////////////////////////////////////////////////
// CreateSnapshotWithData ////////////////////////////////////////////////////

struct CreateSnapshotWithData(PathBuf, Vec<u8>);

impl Message for CreateSnapshotWithData {
    type Result = Result<(), MemoryStorageError>;
}

impl Handler<CreateSnapshotWithData> for SnapshotActor {
    type Result = Result<(), MemoryStorageError>;

    fn handle(&mut self, msg: CreateSnapshotWithData, _: &mut Self::Context) -> Self::Result {
        fs::write(msg.0.clone(), msg.1).map_err(|err| {
            error!("Error writing snapshot file. {}", err);
            MemoryStorageError
        })
    }
}

//////////////////////////////////////////////////////////////////////////////
// DeserializeSnapshot ///////////////////////////////////////////////////////

struct DeserializeSnapshot(PathBuf);

impl Message for DeserializeSnapshot {
    type Result = Result<Vec<Entry>, MemoryStorageError>;
}

impl Handler<DeserializeSnapshot> for SnapshotActor {
    type Result = Result<Vec<Entry>, MemoryStorageError>;

    fn handle(&mut self, msg: DeserializeSnapshot, _: &mut Self::Context) -> Self::Result {
        fs::read(msg.0)
            .map_err(|err| {
                error!("Error reading contents of snapshot file. {}", err);
                MemoryStorageError
            })
            // Deserialize the data of the snapshot file.
            .and_then(|snapdata| {
                rmps::from_slice::<Vec<Entry>>(snapdata.as_slice()).map_err(|err| {
                    error!("Error deserializing snapshot contents. {}", err);
                    MemoryStorageError
                })
            })
    }
}

//////////////////////////////////////////////////////////////////////////////
// SyncInstallSnapshot ///////////////////////////////////////////////////////

impl Message for SyncInstallSnapshot {
    type Result = Result<EntrySnapshotPointer, MemoryStorageError>;
}

impl Handler<SyncInstallSnapshot> for SnapshotActor {
    type Result = Result<EntrySnapshotPointer, MemoryStorageError>;

    fn handle(&mut self, msg: SyncInstallSnapshot, _: &mut Self::Context) -> Self::Result {
        let filename = format!("{}", &msg.0.index);
        let filepath = std::path::PathBuf::from(self.0.clone()).join(filename);

        // Create the new snapshot file.
        let mut snapfile = File::create(&filepath).map_err(|err| {
            error!("Error creating new snapshot file. {}", err);
            MemoryStorageError
        })?;

        let chunk_stream = msg.0.stream.map_err(|_| {
            error!("Snapshot chunk stream hit an error in the memory_storage system.");
            MemoryStorageError
        }).wait();
        let mut did_process_final_chunk = false;
        for chunk in chunk_stream {
            let chunk = chunk?;
            snapfile.seek(SeekFrom::Start(chunk.offset)).map_err(|err| {
                error!("Error seeking to file location for writing snapshot chunk. {}", err);
                MemoryStorageError
            })?;
            snapfile.write_all(&chunk.data).map_err(|err| {
                error!("Error writing snapshot chunk to snapshot file. {}", err);
                MemoryStorageError
            })?;
            if chunk.done {
                did_process_final_chunk = true;
            }
            let _ = chunk.cb.send(());
        }

        if !did_process_final_chunk {
            error!("Prematurely exiting snapshot chunk stream. Never hit final chunk.");
            Err(MemoryStorageError)
        } else {
            Ok(EntrySnapshotPointer{path: filepath.to_string_lossy().to_string()})
        }
    }
}

/*

//////////////////////////////////////////////////////////////////////////////////////////////////
// Other Message Types & Handlers ////////////////////////////////////////////////////////////////
//
// NOTE WELL: these following types, just as the MemoryStorage system overall, is intended
// primarily for testing purposes. Don't build your application using this storage implementation.

/// Get the current state of the storage engine.
pub struct GetCurrentState;

impl Message for GetCurrentState {
    type Result = Result<CurrentStateData, ()>;
}

/// The current state of the storage engine.
pub struct CurrentStateData {
    pub hs: HardState,
    pub log: BTreeMap<u64, Entry>,
    pub snapshot_data: Option<CurrentSnapshotData>,
    pub snapshot_dir: String,
    pub state_machine: BTreeMap<u64, Entry>,
}

impl Handler<GetCurrentState> for MemoryStorage {
    type Result = Result<CurrentStateData, ()>;

    fn handle(&mut self, _: GetCurrentState, _: &mut Self::Context) -> Self::Result {
        Ok(CurrentStateData{
            hs: self.hs.clone(),
            log: self.log.clone(),
            snapshot_data: self.snapshot_data.clone(),
            snapshot_dir: self.snapshot_dir.clone(),
            state_machine: self.state_machine.clone(),
        })
    }
}

*/

pub struct GetNode(pub String);

impl Message for GetNode {
    type Result = Result<Vec<NodeId>, ()>;
}

impl Handler<GetNode> for MemoryStorage {
    type Result = Result<Vec<NodeId>, ()>;

    fn handle(&mut self, msg: GetNode, ctx: &mut Context<Self>) -> Self::Result {
        let mut vec:Vec<NodeId>=Vec::new();
        /*
        let nodeid:NodeId=msg.0.parse().unwrap_or(0);
        if let Some(node_id) = self.add_remove_nodes.get(&nodeid) {
            Ok(*node_id)
        } else {
            Err(())
        }
        */
        for item in self.add_remove_nodes.iter() {
            vec.push(*item);
        }
        Ok(vec)

    }
}


pub struct FindValue(pub Vec<u8>);

impl Message for FindValue {
    type Result = Result<Vec<u8>, Error>;
}

impl Handler<FindValue> for MemoryStorage {
    type Result = Result<Vec<u8>, Error>;

    fn handle(&mut self, msg: FindValue, ctx: &mut Context<Self>) -> Self::Result {
     let mut v="003".to_string().into_bytes();
     v.extend(msg.0);
     get(self.db.clone(),&v)

   
   

    }
}
