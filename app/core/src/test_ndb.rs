use nostr_sdk::{prelude::Event, JsonUtil};
use nostrdb::{Config, Filter, Ndb};

pub fn isolated_ndb(mapsize: usize) -> (Ndb, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("ndb");
    std::fs::create_dir_all(&path).expect("mkdir");
    let cfg = Config::new().set_mapsize(mapsize);
    let ndb = Ndb::new(path.to_str().expect("utf8 ndb path"), &cfg).expect("open ndb");
    (ndb, tmp)
}

pub fn process_event_and_wait(ndb: &Ndb, event: &Event) {
    let id_bytes: [u8; 32] = event.id.to_bytes();
    let filter = Filter::new().ids([&id_bytes]).build();
    let sub = ndb
        .subscribe(&[filter])
        .expect("subscribe for ingested event");
    let line = format!("[\"EVENT\",\"sub\",{}]", event.as_json());

    ndb.process_event(&line).expect("process event");
    futures::executor::block_on(ndb.wait_for_notes(sub, 1)).expect("ingest event");
}
