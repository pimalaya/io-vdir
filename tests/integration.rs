//! End-to-end Vdir test flow.
//!
//! Drives the full [`VdirClient`] surface against a freshly created
//! tempdir.

use std::{collections::BTreeSet, fs, io, path::Path};

use io_vdir::{
    client::{VdirClient, VdirClientError},
    collection::VdirCollection,
    item::{VdirItem, VdirItemKind, store::*},
    path::VdirPath,
};
use tempfile::{TempDir, tempdir};

#[test]
fn end_to_end() {
    let _ = env_logger::try_init();

    let dir = tempdir().expect("create tempdir");
    let root = VdirPath::new(dir.path().to_string_lossy().into_owned());
    let client = VdirClient::new(root.clone());

    // baseline: no collections
    let collections = client
        .list_collections()
        .expect("list collections (baseline)");
    assert!(collections.is_empty(), "root should be empty initially");

    // create two collections
    let contacts = VdirCollection {
        path: root.join("contacts"),
        display_name: Some("Contacts".into()),
        description: Some("Personal contacts".into()),
        color: Some("#3366ff".into()),
    };
    let work = VdirCollection {
        path: root.join("work"),
        display_name: None,
        description: None,
        color: None,
    };

    client
        .create_collection(contacts.clone())
        .expect("create contacts");
    client.create_collection(work.clone()).expect("create work");

    assert!(Path::new(contacts.path.as_str()).is_dir());
    assert!(Path::new(work.path.as_str()).is_dir());

    // list collections; metadata loaded
    let listed = client.list_collections().expect("list collections");
    assert_eq!(listed.len(), 2);
    assert!(
        listed.contains(&contacts),
        "expected contacts in {listed:?}"
    );
    assert!(listed.contains(&work), "expected work in {listed:?}");

    // update metadata
    let mut work_updated = work.clone();
    work_updated.display_name = Some("Work".into());
    work_updated.color = Some("#cc3300".into());
    client
        .update_collection(work_updated.clone())
        .expect("update work");

    let listed = client.list_collections().expect("relist collections");
    let work_back = listed
        .iter()
        .find(|c| c.path == work.path)
        .expect("work present");
    assert_eq!(work_back.display_name.as_deref(), Some("Work"));
    assert_eq!(work_back.color.as_deref(), Some("#cc3300"));

    // store an item with a caller-provided id
    let (id_a, path_a) = client
        .store_item(
            contacts.path.clone(),
            Some(String::from("alice")),
            VdirItemKind::Vcard,
            b"BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Alice\r\nEND:VCARD\r\n".to_vec(),
        )
        .expect("store alice");
    assert_eq!(id_a, "alice");
    assert!(path_a.as_str().ends_with("/contacts/alice.vcf"));
    assert!(Path::new(path_a.as_str()).is_file());

    // store another item with an auto-generated id
    let (id_b, path_b) = client
        .store_item(
            contacts.path.clone(),
            None,
            VdirItemKind::Vcard,
            b"BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Bob\r\nEND:VCARD\r\n".to_vec(),
        )
        .expect("store bob");
    assert_eq!(id_b.len(), 36, "expected a UUIDv4 string, got {id_b}");
    assert!(Path::new(path_b.as_str()).is_file());

    // list items
    let items = client
        .list_items(contacts.path.clone())
        .expect("list items");
    assert_eq!(items.len(), 2);

    // locate alice
    let (located_path, kind) = client
        .locate_item(contacts.path.clone(), "alice")
        .expect("locate alice");
    assert_eq!(located_path, path_a);
    assert!(matches!(kind, VdirItemKind::Vcard));

    // get alice and verify contents round-trip
    let alice = client
        .get_item(contacts.path.clone(), "alice")
        .expect("get alice");
    assert_eq!(alice.path, path_a);
    assert!(matches!(alice.kind, VdirItemKind::Vcard));
    assert!(alice.contents.starts_with(b"BEGIN:VCARD"));

    // overwrite alice via store_item with the same id
    client
        .store_item(
            contacts.path.clone(),
            Some(String::from("alice")),
            VdirItemKind::Vcard,
            b"BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Alice Smith\r\nEND:VCARD\r\n".to_vec(),
        )
        .expect("overwrite alice");
    let alice = client
        .get_item(contacts.path.clone(), "alice")
        .expect("re-get alice");
    assert!(alice.contents.windows(11).any(|w| w == b"FN:Alice Sm"));

    // copy alice to work
    client
        .copy_item(contacts.path.clone(), work.path.clone(), "alice")
        .expect("copy alice");
    let items = client.list_items(work.path.clone()).expect("list work");
    assert_eq!(items.len(), 1);
    assert_eq!(
        file_names(&work.path),
        ["alice.vcf", "color", "displayname"],
        "the copy must leave no staged file behind",
    );

    // move bob to work
    client
        .move_item(contacts.path.clone(), work.path.clone(), &id_b)
        .expect("move bob");
    let items = client.list_items(work.path.clone()).expect("relist work");
    assert_eq!(items.len(), 2);
    let items = client
        .list_items(contacts.path.clone())
        .expect("relist contacts");
    assert_eq!(items.len(), 1, "contacts should only carry alice");

    // delete alice from contacts
    client
        .delete_item(contacts.path.clone(), "alice")
        .expect("delete alice");
    let items = client
        .list_items(contacts.path.clone())
        .expect("list after delete");
    assert!(items.is_empty());

    // rename work to archive
    client
        .rename_collection(work.path.clone(), "archive")
        .expect("rename work");
    let archive_path = root.join("archive");
    assert!(Path::new(archive_path.as_str()).is_dir());

    // delete contacts
    client
        .delete_collection(contacts.path.clone())
        .expect("delete contacts");
    let listed = client.list_collections().expect("final list");
    assert_eq!(listed.len(), 1);
    assert!(listed.iter().any(|c| c.path == archive_path));
}

#[test]
fn ids_and_names_stay_in_their_directory() {
    let dir = tempdir().expect("create tempdir");
    let root = VdirPath::new(dir.path().to_string_lossy().into_owned());
    let client = VdirClient::new(root.clone());

    let contacts = root.join("contacts");
    let work = root.join("work");
    client
        .create_collection(VdirCollection::from_path(contacts.clone()))
        .expect("create contacts");
    client
        .create_collection(VdirCollection::from_path(work.clone()))
        .expect("create work");

    let (_, bob) = client
        .store_item(
            work.clone(),
            Some(String::from("bob")),
            VdirItemKind::Vcard,
            b"BEGIN:VCARD\r\nEND:VCARD\r\n".to_vec(),
        )
        .expect("store bob");

    client
        .store_item(
            contacts.clone(),
            Some(String::from("../escaped")),
            VdirItemKind::Vcard,
            b"BEGIN:VCARD\r\nEND:VCARD\r\n".to_vec(),
        )
        .expect_err("store outside the collection");
    assert!(!Path::new(root.join("escaped.vcf").as_str()).exists());

    client
        .delete_item(contacts.clone(), "../work/bob")
        .expect_err("delete outside the collection");
    assert!(Path::new(bob.as_str()).is_file());

    client
        .move_item(work.clone(), contacts.clone(), "../work/bob")
        .expect_err("move through a foreign path");
    assert!(Path::new(bob.as_str()).is_file());

    client
        .rename_collection(contacts.clone(), "work/nested")
        .expect_err("rename outside the parent");
    assert!(Path::new(contacts.as_str()).is_dir());
    assert!(!Path::new(work.join("nested").as_str()).exists());
}

#[test]
fn entries_that_are_not_files_are_not_listed() {
    let dir = tempdir().expect("create tempdir");
    let (client, contacts) = seeded(&dir, &["alice"]);

    fs::create_dir(contacts.join("folder.ics").as_str()).expect("create folder.ics");
    #[cfg(unix)]
    std::os::unix::fs::symlink("missing", contacts.join("dangling.vcf").as_str())
        .expect("create dangling.vcf");

    let items = client.list_items(contacts.clone()).expect("list items");
    assert_eq!(ids(&items), ["alice"]);
}

#[cfg(unix)]
#[test]
fn unreadable_item_fails_the_listing() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().expect("create tempdir");
    let (client, contacts) = seeded(&dir, &["alice", "bob"]);

    let alice = contacts.join("alice.vcf");
    fs::set_permissions(alice.as_str(), fs::Permissions::from_mode(0o000)).expect("lock alice");

    // root reads whatever the mode says
    if fs::read(alice.as_str()).is_ok() {
        return;
    }

    match client.list_items(contacts.clone()) {
        Err(VdirClientError::FileRead { path, err }) => {
            assert_eq!(path, alice);
            assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        }
        other => panic!("expected FileRead, got {other:?}"),
    }

    let (items, unreadable) = client
        .list_readable_items(contacts.clone())
        .expect("list readable items");
    assert_eq!(ids(&items), ["bob"]);

    let unreadable: Vec<_> = unreadable
        .iter()
        .map(|(entry, err)| (entry.id.as_str(), err.kind()))
        .collect();
    assert_eq!(unreadable, [("alice", io::ErrorKind::PermissionDenied)]);
}

#[test]
fn vanished_item_is_skipped() {
    let dir = tempdir().expect("create tempdir");
    let (client, contacts) = seeded(&dir, &["alice", "bob"]);

    let entries = client
        .list_item_entries(contacts.clone())
        .expect("list entries");
    fs::remove_file(contacts.join("alice.vcf").as_str()).expect("remove alice");

    let items = client.read_items(entries.clone()).expect("read items");
    assert_eq!(ids(&items), ["bob"]);

    let (items, unreadable) = client
        .read_readable_items(entries)
        .expect("read readable items");
    assert_eq!(ids(&items), ["bob"]);

    let unreadable: Vec<_> = unreadable
        .iter()
        .map(|(entry, err)| (entry.id.as_str(), err.kind()))
        .collect();
    assert_eq!(unreadable, [("alice", io::ErrorKind::NotFound)]);
}

#[test]
fn entry_that_cannot_be_read_fails_the_batch() {
    let dir = tempdir().expect("create tempdir");
    let (client, contacts) = seeded(&dir, &["alice", "bob"]);

    let entries = client
        .list_item_entries(contacts.clone())
        .expect("list entries");

    // a directory took the place of the listed file: not a deletion
    let alice = contacts.join("alice.vcf");
    fs::remove_file(alice.as_str()).expect("remove alice");
    fs::create_dir(alice.as_str()).expect("create a directory in its place");

    match client.read_items(entries.clone()) {
        Err(VdirClientError::FileRead { path, .. }) => assert_eq!(path, alice),
        other => panic!("expected FileRead, got {other:?}"),
    }

    let (items, unreadable) = client
        .read_readable_items(entries)
        .expect("read readable items");
    assert_eq!(ids(&items), ["bob"]);

    let unreadable: Vec<_> = unreadable.keys().map(|entry| entry.id.as_str()).collect();
    assert_eq!(unreadable, ["alice"]);
}

#[test]
fn entries_are_read_in_batches() {
    let dir = tempdir().expect("create tempdir");
    let (client, contacts) = seeded(&dir, &["alice", "bob", "carol"]);

    let entries: Vec<_> = client
        .list_item_entries(contacts.clone())
        .expect("list entries")
        .into_iter()
        .collect();
    let ids: Vec<_> = entries.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(ids, ["alice", "bob", "carol"]);

    for batch in entries.chunks(2) {
        let items = client.read_items(batch.to_vec()).expect("read batch");

        let read: Vec<_> = items.iter().map(|item| (&item.path, item.meta)).collect();
        let listed: Vec<_> = batch
            .iter()
            .map(|entry| (&entry.path, entry.meta))
            .collect();
        assert_eq!(read, listed);
    }
}

#[test]
fn conditional_stores_check_the_item_on_disk() {
    let dir = tempdir().expect("create tempdir");
    let root = VdirPath::new(dir.path().to_string_lossy().into_owned());
    let client = VdirClient::new(root.clone());

    let contacts = root.join("contacts");
    let store = |mode, contents: &[u8]| {
        client.run(VdirItemStore::new(
            contacts.clone(),
            Some(String::from("alice")),
            VdirItemKind::Vcard,
            contents.to_vec(),
            VdirItemStoreOptions { mode },
        ))
    };

    store(VdirItemStoreMode::IfAbsent, b"first").expect("create alice");
    let err = store(VdirItemStoreMode::IfAbsent, b"second").expect_err("create alice twice");
    assert!(matches!(
        err,
        VdirClientError::VdirItemStore(VdirItemStoreError::Exists(_))
    ));

    let read = client
        .get_item(contacts.clone(), "alice")
        .expect("get alice");
    assert_eq!(read.contents, b"first");

    // another writer gets in between the read and the store
    store(VdirItemStoreMode::Replace, b"someone else").expect("replace alice");

    let err = store(VdirItemStoreMode::IfUnchanged(read.meta), b"mine").expect_err("stale store");
    assert!(matches!(
        err,
        VdirClientError::VdirItemStore(VdirItemStoreError::Changed(_))
    ));

    let read = client
        .get_item(contacts.clone(), "alice")
        .expect("get alice");
    assert_eq!(read.contents, b"someone else");
    assert_eq!(file_names(&contacts), ["alice.vcf"]);

    store(VdirItemStoreMode::IfUnchanged(read.meta), b"mine").expect("fresh store");
    let read = client
        .get_item(contacts.clone(), "alice")
        .expect("get alice");
    assert_eq!(read.contents, b"mine");
}

/// Builds a client over `dir` with the given vCards in `contacts`.
fn seeded(dir: &TempDir, ids: &[&str]) -> (VdirClient, VdirPath) {
    let root = VdirPath::new(dir.path().to_string_lossy().into_owned());
    let client = VdirClient::new(root.clone());
    let contacts = root.join("contacts");

    for id in ids {
        client
            .store_item(
                contacts.clone(),
                Some(String::from(*id)),
                VdirItemKind::Vcard,
                format!("BEGIN:VCARD\r\nFN:{id}\r\nEND:VCARD\r\n").into_bytes(),
            )
            .expect("store item");
    }

    (client, contacts)
}

fn ids(items: &BTreeSet<VdirItem>) -> Vec<&str> {
    items.iter().filter_map(VdirItem::id).collect()
}

fn file_names(dir: &VdirPath) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir.as_str())
        .expect("read dir")
        .map(|entry| entry.expect("dir entry").file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}
