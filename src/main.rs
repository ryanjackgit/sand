use actix::prelude::*;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws;
use actix_files as fs;
use std::env;
use futures::{Future};

use sand::network::{Network, GetNode,Save,Find};


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
    let mut net = Network::new();

    let args: Vec<String> = env::args().collect();
    let local_address = args[1].as_str();
    let public_address = args[2].as_str();

    // listen on ip and port
    net.listen(local_address);

    // register peers
    let peers = vec![
        "127.0.0.1:8000",
        "127.0.0.1:8001",
        "127.0.0.1:8002",
    ];

    net.peers(peers);

    let net_addr = net.start();

    HttpServer::new(move || {
        App::new()
            .data(net_addr.clone())
            .service(web::resource("/node/{uid}").to_async(index_route))
            .service(web::resource("/save/{uid}").to_async(save_route))
            .service(web::resource("/find/{uid}").to_async(find_route))
        // static resources
            .service(fs::Files::new("/static/", "static/"))
    })
        .bind(public_address)
        .unwrap()
        .start();

    let _ = sys.run();
}
