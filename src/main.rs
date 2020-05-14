use actix::prelude::*;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws;
use actix_files as fs;
use std::env;
use futures::{Future};
use actix_raft::NodeId;

use sand::network::{Network,NetworkState, GetNode,GetNodes,GetClusterState,Save,Find,ChangeRaftClusterConfig};
use serde::{Serialize,Deserialize};
use std::fs::File;
use std::io::Read;

fn index_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<Network>>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    let uid = req.match_info().get("uid").unwrap_or("");

    srv.send(GetNode(uid.to_string()))
        .map_err(Error::from)
        .and_then(|res| {
            Ok(HttpResponse::Ok().body(res.unwrap().to_string()))
        })
}


fn state_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<Network>>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    srv
        .send(GetClusterState)
        .map_err(Error::from)
        .and_then(|res| Ok(HttpResponse::Ok().json(res)))
}

fn join_cluster_route(
    node_id: web::Json<NodeId>,
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<Network>>,
) ->  HttpResponse {

    println!("got join request with id {:#?}", node_id);
    srv.do_send(ChangeRaftClusterConfig(vec![*node_id], vec![]));
    HttpResponse::Ok().json(()) // <- send json response
}


fn getNodes_route(
     req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<Network>>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    srv
        .send(GetNodes)
        .map_err(Error::from)
        .and_then(|res| Ok(HttpResponse::Ok().json(res)))
}


fn save_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<Network>>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    let uid = req.match_info().get("uid").unwrap_or("0");
    let uid=uid.parse::<u64>().unwrap();
    println!("the save uid is ---------------{}",uid);
    srv.send(Save(uid))
        .map_err(Error::from)
        .and_then(|res| {
            Ok(HttpResponse::Ok().body("have handle".to_string()))
        })
}


fn find_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<Network>>,
) -> impl Future<Item = HttpResponse, Error = Error> {
    let uid = req.match_info().get("uid").unwrap_or("0");
    let uid=uid.parse::<u64>().unwrap();
    println!("the find uid is ---------------{}",uid);
    srv.send(Find(uid))
        .map_err(Error::from)
        .and_then(|res| {
            Ok(HttpResponse::Ok().body(res.unwrap().to_string()))
        })
}


fn main() {
    let sys = System::new("testing");

    #[derive(Serialize, Deserialize)]
    pub struct Config {
    discovery_server: String,
   }


   let mut file = File::open("config.json").unwrap();
   let mut buff = String::new();
   file.read_to_string(&mut buff).unwrap();

   let foo: Config = serde_json::from_str(&buff).unwrap();
   //println!("Name: {}", foo.name);



    let register_server=foo.discovery_server.clone();

    println!("the resister server is {}",register_server);

    let mut net = Network::new(register_server);

    let args: Vec<String> = env::args().collect();
    let local_address = args[1].as_str();
    let public_address = args[2].as_str();

    // listen on ip and port
    net.listen(local_address);

    // register init static member peers
    let peers = vec![
        "127.0.0.1:8000",
        "127.0.0.1:8001",
       ];

    net.peers(peers);

    let net_addr = net.start();

    HttpServer::new(move || {
        App::new()
            .data(net_addr.clone())
            .service(web::resource("/node/{uid}").to_async(index_route))
            .service(web::resource("/save/{uid}").to_async(save_route))
            .service(web::resource("/find/{uid}").to_async(find_route))
              .service(web::resource("/cluster/state").to_async(state_route))
            .service(web::resource("/cluster/nodes").to_async(getNodes_route))
            .service(web::resource("/cluster/join").route(web::put().to_async(join_cluster_route)))
        // static resources  getNodes_route
           // .service(fs::Files::new("/static/", "static/"))
    })
        .bind(public_address)
        .unwrap()
        .start();

    let _ = sys.run();
}
