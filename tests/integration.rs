//! End-to-end Vdir test flow.
//!
//! Drives the full [`VdirClient`] surface against a freshly created
//! tempdir.

use std::path::Path;

use io_vdir::{client::VdirClient, collection::Collection, item::ItemKind, path::VdirPath};
use tempfile::tempdir;

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
    let contacts = Collection {
        path: root.join("contacts"),
        display_name: Some("Contacts".into()),
        description: Some("Personal contacts".into()),
        color: Some("#3366ff".into()),
    };
    let work = Collection {
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
            ItemKind::Vcard,
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
            ItemKind::Vcard,
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
    assert!(matches!(kind, ItemKind::Vcard));

    // get alice and verify contents round-trip
    let alice = client
        .get_item(contacts.path.clone(), "alice")
        .expect("get alice");
    assert_eq!(alice.path, path_a);
    assert!(matches!(alice.kind, ItemKind::Vcard));
    assert!(alice.contents.starts_with(b"BEGIN:VCARD"));

    // overwrite alice via store_item with the same id
    client
        .store_item(
            contacts.path.clone(),
            Some(String::from("alice")),
            ItemKind::Vcard,
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
