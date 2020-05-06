# sand 

Rust Actor(Actix)  raft  implement Distributed storage system.

How to test it?

1,git clone https://github.com/ryanjackgit/sand

2, cd sand 
 cargo build

3, in main.rs  set  start_init_state =  SingleNode
start alone as single node server :

./target/debug/sand 127.0.0.1:8000 127.0.0.1:9000
then please wait 15 seconds,the single node cluser works.

4, if in main.rs set start_init_state =  NetworkState::Cluster  staic cluser
 start 3 nodes cluster:(attention!  must start the three node in 10 seconds)

./target/debug/sand 127.0.0.1:8000 127.0.0.1:9000


./target/debug/sand 127.0.0.1:8001 127.0.0.1:9001

./target/debug/sand 127.0.0.1:8002 127.0.0.1:9002

8000,8001,8002 is a port about internal comunnication,9000 etc is http port

5. test the cluster running state:

http://127.0.0.1:9000/save/{{integer}}  wirte  integer to cluster.

http://127.0.0.1:9000/find/{{integer}}  verify,if integer in cluser.it return true,if not in cluster,return false.


