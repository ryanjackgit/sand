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
            let s=format!("the all nodes is {:?}",res.unwrap());
            Ok(HttpResponse::Ok().body(s))
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
    let key = req.match_info().get("key").unwrap_or("0");
    let value = req.match_info().get("value").unwrap_or("0");
    println!("the save uid is ---------------{}:{}",key,value);
    srv.send(Save(key.to_string(),value.to_string()))
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
    use rmp_serde as rmps;
    println!("the find uid is ---------------{}",&uid);
    let key=uid.to_string();
    let key=key.into_bytes();
    srv.send(Find(key))
        .map_err(Error::from)
        .and_then(|res| {
            match res {
                Ok(s) => {
                    let res:String=rmps::from_read_ref(&s).unwrap();
                    Ok(HttpResponse::Ok().body(res))
                }
                Err(_x) => {
                    let res:String="don't find your key".to_string();
                    Ok(HttpResponse::Ok().body(res))
                }
            }

            //let s=String.from_utf8(res.unwrap()).unwrap_or("not find".to_string());
        
        })
}

fn index(req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<Network>>) ->  impl Future<Item = HttpResponse, Error = Error>  {
    let html = r#"<html>
        <head><title>Upload Test</title></head>
        <body>
            <form target="/" method="post" enctype="multipart/form-data">
                <input type="file" multiple name="file"/>
                <input type="submit" value="Submit"></button>
            </form>
        </body>
    </html>"#;
    use futures::future::ok;
    ok(HttpResponse::Ok().body(html))
}

use std::io::Write;


//use actix_web::{middleware, web, App, Error, HttpResponse, HttpServer};
//use futures::{StreamExt, TryStreamExt};

fn save_file(req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<Network>>,) -> impl Future<Item = HttpResponse, Error = Error> {
    // iterate over multipart stream
    /*
    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_type = field.content_disposition().unwrap();
        let filename = content_type.get_filename().unwrap();
        let filepath = format!("./tmp/{}", sanitize_filename::sanitize(&filename));
        // File::create is blocking operation, use threadpool
        let mut f = web::block(|| std::fs::File::create(filepath))
            .await
            .unwrap();
        // Field in turn is stream of *Bytes* object
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            // filesystem operations are blocking, we have to use threadpool
            f = web::block(move || f.write_all(&data).map(|_| f)).await?;
        }
    }
    */
    use futures::future::ok;
    ok(HttpResponse::Ok().into())
}


fn main() {
    let sys = System::new("testing");

    #[derive(Serialize, Deserialize)]
    pub struct Config {
    discovery_server: String,
    nodes:Vec<String>,
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
    let static_peers =foo.nodes.clone();
    println!("the static_nodes is {:?}",&static_peers);

    net.peers(static_peers);

    let net_addr = net.start();

    HttpServer::new(move || {
        App::new()
            .data(net_addr.clone())
            .service(web::resource("/node/{uid}").to_async(index_route))
            .service(web::resource("/put/{key}/{value}").to_async(save_route))
            .service(web::resource("/get/{uid}").to_async(find_route))
            .service(web::resource("/cluster/state").to_async(state_route))
            .service(web::resource("/cluster/nodes").to_async(getNodes_route))
            .service(web::resource("/cluster/join").route(web::put().to_async(join_cluster_route)))
            .service(web::resource("/").route(web::get().to_async(index)).route(web::post().to_async(save_file)))
        // static resources  getNodes_route
           // .service(fs::Files::new("/static/", "static/"))
    })
        .bind(public_address)
        .unwrap()
        .start();

    let _ = sys.run();
}
