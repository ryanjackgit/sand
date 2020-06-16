# sand 

 This version support RocksDB as storage.

 this version support first start as singlenode,after other node join this single node cluster,
 attention, this style started cluster must keep two  nodes at least.

This implemention include Rust Actor(actix-raft) model raft  implemention and RocksDB as storage implemention,RocksDB save the raft states and Key-Value data.the default path is /tmp/rocksdb.

prepare for 3 or 5 nodes server,please use docker,ips is :172.17.0.1,172.17.0.2,172.17.0.3,172.17.0.4
172.17.0.5.

1, require build RocksDB lib,please google search to build RocksDB lib.                         

2,git clone https://github.com/ryanjackgit/sand

3, cd sand 
 cargo build

4, 
first start alone as single node server :

./target/debug/sand 172.17.0.1:8000 172.17.0.1:9000
then please wait 5 seconds,the single node cluser works.

5,  in sand/config.json,set  discovery_server="172.17.0.1:9000" as discovery node . 
 
 start 3 nodes cluster: of course ,you may add more.



./target/debug/sand 172.17.0.2:8000 172.17.0.2:9000

./target/debug/sand 172.17.0.3:8000 172.17.0.3:9000

./target/debug/sand 172.17.0.4:8000 172.17.0.4:9000

./target/debug/sand 172.17.0.5:8000 172.17.0.5:9000

8000 is a port about raft  internal comunnication,9000  is http port.

6. test the cluster running state: 

http://172.17.0.1:9000/put/{{key}}/{{value}}  wirte  Key-value to cluster.

http://127.0.0.1:9000/get/{{Key}}  verify,if key in this node.it return true,if not in cluster,return false.


of course,you may test every node see if or not it save this key-value data

说明：
1，Actix-raft 是Rust中实现Raft规范协议比较完全的一个，基本实现了所有标准规范的所有功能，可以投入产品中使用。
2，分布式的Key-Value主要使用了muti raft,对Key使用范围分区或哈西分区来实现分布式存储。主要包括分区的重新划分和重新聚集分片。
