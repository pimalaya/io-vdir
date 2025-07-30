use std::{collections::HashSet, io::ErrorKind};

use calcard::vcard::VCard;
use io_fs::runtimes::std::handle;
use io_vdir::{
    collection::Collection,
    coroutines::{
        create_collection::{CreateCollection, CreateCollectionResult},
        create_item::{CreateItem, CreateItemResult},
        delete_collection::{DeleteCollection, DeleteCollectionResult},
        delete_item::{DeleteItem, DeleteItemResult},
        list_collections::{ListCollections, ListCollectionsResult},
        list_items::{ListItems, ListItemsResult},
        update_collection::{UpdateCollection, UpdateCollectionResult},
        update_item::{UpdateItem, UpdateItemResult},
    },
    item::{Item, ItemKind},
};
use tempfile::tempdir;

#[test]
fn std() {
    let workdir = tempdir().unwrap();
    let root = workdir.path();

    // should list empty collections

    let mut arg = None;
    let mut list = ListCollections::new(&root);

    let collections = loop {
        match list.resume(arg) {
            ListCollectionsResult::Ok(collections) => break collections,
            ListCollectionsResult::Io(io) => arg = Some(handle(io).unwrap()),
            ListCollectionsResult::Err(err) => panic!("{err}"),
        }
    };

    assert!(collections.is_empty());

    // should create collection without metadata

    let mut collection = Collection::new(&root);

    let mut arg = None;
    let mut create = CreateCollection::new(collection.clone());

    loop {
        match create.resume(arg) {
            CreateCollectionResult::Ok => break,
            CreateCollectionResult::Io(io) => arg = Some(handle(io).unwrap()),
            CreateCollectionResult::Err(err) => panic!("{err}"),
        }
    }

    let mut arg = None;
    let mut list = ListCollections::new(&root);

    let collections = loop {
        match list.resume(arg) {
            ListCollectionsResult::Ok(collections) => break collections,
            ListCollectionsResult::Io(io) => arg = Some(handle(io).unwrap()),
            ListCollectionsResult::Err(err) => panic!("{err}"),
        }
    };

    let expected_collections = HashSet::from_iter([collection.clone()]);

    assert_eq!(collections, expected_collections);

    // should not re-create existing collection

    let mut arg = None;
    let mut create = CreateCollection::new(collection.clone());

    loop {
        match create.resume(arg) {
            CreateCollectionResult::Ok => panic!("should fail"),
            CreateCollectionResult::Io(io) => match handle(io) {
                Ok(output) => arg = Some(output),
                Err(err) => break assert_eq!(err.kind(), ErrorKind::AlreadyExists),
            },
            CreateCollectionResult::Err(err) => panic!("{err}"),
        }
    }

    // should update collection with metadata

    collection.display_name = Some("Custom collection name".into());
    collection.description = Some("This is a description.".into());
    collection.color = Some("#000000".into());

    let mut arg = None;
    let mut update = UpdateCollection::new(collection.clone());

    loop {
        match update.resume(arg) {
            UpdateCollectionResult::Ok => break,
            UpdateCollectionResult::Io(io) => arg = Some(handle(io).unwrap()),
            UpdateCollectionResult::Err(err) => panic!("{err}"),
        }
    }

    let mut arg = None;
    let mut list = ListCollections::new(&root);

    let collections = loop {
        match list.resume(arg) {
            ListCollectionsResult::Ok(items) => break items,
            ListCollectionsResult::Io(io) => arg = Some(handle(io).unwrap()),
            ListCollectionsResult::Err(err) => panic!("{err}"),
        }
    };

    let expected_collections = HashSet::from_iter([collection.clone()]);

    assert_eq!(collections, expected_collections);

    // should create item

    let mut item = Item::new(
        &collection,
        ItemKind::Vcard(VCard::parse("BEGIN:VCARD\r\nUID: abc123\r\nEND:VCARD\r\n").unwrap()),
    );

    let mut arg = None;
    let mut create = CreateItem::new(item.clone());

    loop {
        match create.resume(arg) {
            CreateItemResult::Ok => break,
            CreateItemResult::Io(io) => arg = Some(handle(io).unwrap()),
            CreateItemResult::Err(err) => panic!("{err}"),
        }
    }

    let mut arg = None;
    let mut list = ListItems::new(&collection);

    let items = loop {
        match list.resume(arg) {
            ListItemsResult::Ok(items) => break items,
            ListItemsResult::Io(io) => arg = Some(handle(io).unwrap()),
            ListItemsResult::Err(err) => panic!("{err}"),
        }
    };

    assert_eq!(items.len(), 1);

    let first_item = items.into_iter().next().unwrap();

    assert_eq!(
        first_item.to_string(),
        "BEGIN:VCARD\r\nVERSION:4.0\r\nUID: abc123\r\nEND:VCARD\r\n"
    );

    // should update item

    item.kind =
        ItemKind::Vcard(VCard::parse("BEGIN:VCARD\r\nUID: def456\r\nEND:VCARD\r\n").unwrap());

    let mut arg = None;
    let mut update = UpdateItem::new(item);

    loop {
        match update.resume(arg) {
            UpdateItemResult::Ok => break,
            UpdateItemResult::Io(io) => arg = Some(handle(io).unwrap()),
            UpdateItemResult::Err(err) => panic!("{err}"),
        }
    }

    let mut arg = None;
    let mut list = ListItems::new(&collection);

    let items = loop {
        match list.resume(arg) {
            ListItemsResult::Ok(items) => break items,
            ListItemsResult::Io(io) => arg = Some(handle(io).unwrap()),
            ListItemsResult::Err(err) => panic!("{err}"),
        }
    };

    assert_eq!(items.len(), 1);

    let item = items.into_iter().next().unwrap();

    assert_eq!(
        item.to_string(),
        "BEGIN:VCARD\r\nVERSION:4.0\r\nUID: def456\r\nEND:VCARD\r\n"
    );

    // // should read item

    // let mut output = None;
    // let mut fs = ReadItem::vcard(collection.path(), "item");

    // let expected_item = loop {
    //     match fs.resume(output) {
    //         Ok(item) => break item,
    //         Err(input) => output = Some(handle(input).unwrap()),
    //     }
    // };

    // assert_eq!(item, expected_item);

    // should delete item

    let mut arg = None;
    let mut delete = DeleteItem::new(item);

    loop {
        match delete.resume(arg) {
            DeleteItemResult::Ok => break,
            DeleteItemResult::Io(io) => arg = Some(handle(io).unwrap()),
            DeleteItemResult::Err(err) => panic!("{err}"),
        }
    }

    let mut arg = None;
    let mut list = ListItems::new(&collection);

    let items = loop {
        match list.resume(arg) {
            ListItemsResult::Ok(items) => break items,
            ListItemsResult::Io(io) => arg = Some(handle(io).unwrap()),
            ListItemsResult::Err(err) => panic!("{err}"),
        }
    };

    assert_eq!(items.into_iter().count(), 0);

    // should delete collection

    let mut arg = None;
    let mut delete = DeleteCollection::new(&collection);

    loop {
        match delete.resume(arg) {
            DeleteCollectionResult::Ok => break,
            DeleteCollectionResult::Io(io) => arg = Some(handle(io).unwrap()),
            DeleteCollectionResult::Err(err) => panic!("{err}"),
        }
    }

    let mut arg = None;
    let mut list = ListCollections::new(root);

    let collections = loop {
        match list.resume(arg) {
            ListCollectionsResult::Ok(items) => break items,
            ListCollectionsResult::Io(io) => arg = Some(handle(io).unwrap()),
            ListCollectionsResult::Err(err) => panic!("{err}"),
        }
    };

    assert!(collections.is_empty());
}
