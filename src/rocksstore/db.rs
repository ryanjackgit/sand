use rocksdb::{ColumnFamilyDescriptor, MergeOperands, Options, SliceTransform, WriteBatch, DB};

use failure::{Error,format_err};

use log::{info,error};
use std::sync::Arc;

pub fn open_db(path: &str) -> rocksdb::DB {
    use std::cmp::Ordering;

    fn first_three(k: &[u8]) -> &[u8] {
        &k[..3]
    }
  
   fn mycomparator(one:&[u8],two:&[u8]) -> Ordering {
  /*
    let flag_one=&one[..3]==&[0,0,1] && &two[0..3]==&[0,0,1];
    let flag_two=&one[..3]==&[0,0,0] && &two[0..3]==&[0,0,0];
    if  flag_one || flag_two{
        let one_string=String::from_utf8(one[3..].to_vec()).unwrap_or("0".to_string());
        let two_string=String::from_utf8(two[3..].to_vec()).unwrap_or("0".to_string());
        let one_size:u64=one_string.parse().unwrap();
        let two_size:u64=two_string.parse().unwrap();
        one_size.cmp(&two_size)
    } else {
         one.cmp(two)
    }
    */
    /*
    let one_string=String::from_utf8(one[3..].to_vec()).unwrap_or("0".to_string());
    let two_string=String::from_utf8(two[3..].to_vec()).unwrap_or("0".to_string());
    let one_size:u64=one_string.parse().unwrap_or(0);
    let two_size:u64=two_string.parse().unwrap_or(0);
    one_size.cmp(&two_size)
*/
   one.cmp(two)
   }

    let prefix_extractor = SliceTransform::create("first_three", first_three, None);

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.set_comparator("my001",mycomparator);
    opts.set_prefix_extractor(prefix_extractor);

    DB::open(&opts, path).unwrap()
}

pub fn insert(db: Arc<rocksdb::DB>, key: &[u8], value: &[u8]) -> Result<(), Error> {
    match db.put(key, value) {
        Ok(x) => Ok(x),
        Err(r) => Err(r.into()),
    }
}

pub fn get(db: Arc<rocksdb::DB>, key: &[u8]) -> Result<Vec<u8>, Error> {
    match db.get(key) {
        Ok(Some(value)) => Ok(value),
        Ok(None) => Err(format_err!("{}","not find")),
        Err(e) =>   Err(e.into()),
    }
}

pub fn delete(db: Arc<rocksdb::DB>, key: &[u8]) -> Result<(), Error> {
    match db.delete(key) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    }
}



pub fn insert_db(db: &rocksdb::DB, key: &[u8], value: &[u8]) -> Result<(), Error> {
    match db.put(key, value) {
        Ok(x) => Ok(x),
        Err(r) => Err(r.into()),
    }
}

pub fn get_prefix_iterator(db: &rocksdb::DB, prefix: &[u8]) -> Vec<(Box<[u8]>, Box<[u8]>)> {
    db.prefix_iterator(prefix).collect::<Vec<_>>()
}

pub fn test_slice_transform() {
    let path = "_rust_rocksdb_slicetransform_test";
    let a1: Box<[u8]> = key(b"aaa1");
    let a2: Box<[u8]> = key(b"aaa2");
    let b1: Box<[u8]> = key(b"bbb1");
    let b2: Box<[u8]> = key(b"bbb2");

    fn first_three(k: &[u8]) -> &[u8] {
        &k[..3]
    }

    let prefix_extractor = SliceTransform::create("first_three", first_three, None);

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.set_prefix_extractor(prefix_extractor);

    let db = DB::open(&opts, path).unwrap();

    assert!(db.put(&*a1, &*a1).is_ok());
    assert!(db.put(&*a2, &*a2).is_ok());
    assert!(db.put(&*b1, &*b1).is_ok());
    assert!(db.put(&*b2, &*b2).is_ok());

    fn cba(input: &Box<[u8]>) -> Box<[u8]> {
        input.iter().cloned().collect::<Vec<_>>().into_boxed_slice()
    }

    fn key(k: &[u8]) -> Box<[u8]> {
        k.to_vec().into_boxed_slice()
    }

    {
        let expected = vec![(cba(&a1), cba(&a1)), (cba(&a2), cba(&a2))];
        let a_iterator = db.prefix_iterator(b"aaa");
        assert_eq!(a_iterator.collect::<Vec<_>>(), expected)
    }

    {
        let expected = vec![(cba(&b1), cba(&b1)), (cba(&b2), cba(&b2))];
        let b_iterator = db.prefix_iterator(b"bbb");
        assert_eq!(b_iterator.collect::<Vec<_>>(), expected)
    }
}

fn atomic_write_batch(path: &str) {
    info!("begin to open the file ");

    let db = DB::open_default(path).unwrap();

    let mut batch = WriteBatch::default();
    batch.put(b"my key", b"my value");
    batch.put(b"key2", b"value2");
    batch.put(b"key3", b"value3");
    batch.put(b"key4", b"value4");
    db.write(batch); // Atomically commits the batch
    info!("the write is end");
}

fn iterate_sample(path: &str) {
    let mut db = DB::open_default(path).unwrap();

    let mut iter = db.raw_iterator();
    ///
    /// // Forwards iteration
    info!("the first -----");
    iter.seek_to_first();
    while iter.valid() {
        info!("Saw {:?} {:?}", iter.key(), iter.value());
        iter.next();
    }
    ///
    /// // Reverse iteration
    info!("----------------------last and prev");
    iter.seek_to_last();
    while iter.valid() {
        info!("Saw {:?} {:?}", iter.key(), iter.value());
        iter.prev();
    }
    ///
    /// // Seeking
    info!("the concrete iter--------");
    iter.seek(b"my key");
    while iter.valid() {
        info!("Saw {:?} {:?}", iter.key(), iter.value());
        iter.next();
    }
    ///
    /// // Reverse iteration from key
    /// // Note, use seek_for_prev when reversing because if this key doesn't exist,
    /// // this will make the iterator start from the previous key rather than the next.
    info!("seek for pre------");
    iter.seek_for_prev(b"my key");
    while iter.valid() {
        info!("Saw {:?} {:?}", iter.key(), iter.value());
        iter.prev();
    }
}

fn create_cf(cf: &str, path: &str) -> Result<(), Error> {
    // should be able to create column families

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.set_merge_operator("test operator", test_provided_merge, None);
    let mut db = DB::open(&opts, path).unwrap();
    let opts = Options::default();
    match db.create_cf(cf, &opts) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn list_cf(path: &str) {
    let opts = Options::default();
    let vec = DB::list_cf(&opts, path);
    match vec {
        Ok(vec) => info!("list cf is {:?}", vec),
        Err(e) => error!("failed to list column family: {}", e),
    }
}

fn drop_cf1(path: &str) {
    let mut db = DB::open_cf(&Options::default(), path, &["cf1"]).unwrap();
    match db.drop_cf("cf1") {
        Ok(_) => info!("cf1 successfully dropped."),
        Err(e) => panic!("failed to drop column family: {}", e),
    }
}



fn test_provided_merge(
    _: &[u8],
    existing_val: Option<&[u8]>,
    operands: &mut MergeOperands,
) -> Option<Vec<u8>> {
    let nops = operands.size_hint().0;
    let mut result: Vec<u8> = Vec::with_capacity(nops);
    match existing_val {
        Some(v) => {
            for e in v {
                result.push(*e);
            }
        }
        None => (),
    }
    for op in operands {
        for e in op {
            result.push(*e);
        }
    }
    Some(result)
}
