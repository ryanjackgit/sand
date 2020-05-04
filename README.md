# sand 

Rust Actor(Actix)  Rocksdb  implement Distributed storage system.

How to test it?

1,git clone https://github.com/ryanjackgit/sand

2, cd sand 
 cargo build

3,
first, start 3 nodes:

./target/debug/sand 127.0.0.1:8000 127.0.0.1:9000


./target/debug/sand 127.0.0.1:8001 127.0.0.1:9001

./target/debug/sand 127.0.0.1:8002 127.0.0.1:9002

8000,8001,8002 is a port about internal comunnicatin,9000 etc is http port

4,
http://127.0.0.1:9000/save/{{integer}}  wirte  integer to cluster.
http://127.0.0.1:9000/find/{{integer}}  verify,if integer in cluser.


