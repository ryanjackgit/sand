# sand 
 this version support first start as singlenode,after other node join this single node cluster,
 attention, this style started cluster must keep two  nodes at least.

Rust Actor(actix-raft) model raft  implement test 

How to test it ?

1,git clone https://github.com/ryanjackgit/sand

2, cd sand 
 cargo build

3, 
first start alone as single node server :

./target/debug/sand 127.0.0.1:8000 127.0.0.1:9000
then please wait 5 seconds,the single node cluser works.

4,  in src/config.json,set  discovery_server="127.0.0.1:9000" as discovery node . 
 
 start 3 nodes cluster: of course ,you may add more.



./target/debug/sand 127.0.0.1:8001 127.0.0.1:9001

./target/debug/sand 127.0.0.1:8002 127.0.0.1:9002

./target/debug/sand 127.0.0.1:8003 127.0.0.1:9003

8000,8001,8002 is a port about raft  internal comunnication,9000 9002 9003 etc is http port

5. test the cluster running state:  {{integer}} is a interger,for instance http://127.0.0.1:9000/save/23,it will save 23 to cluster

http://127.0.0.1:9000/save/{{integer}}  wirte  integer to cluster.

http://127.0.0.1:9000/find/{{integer}}  verify,if integer in this node.it return true,if not in cluster,return false.

http://127.0.0.1:9001/find/{{integer}}  verify,if integer in this node.it return true,if not in cluster,return false.

of course,you may test every node see if or not it save this integer


