//! Example: store an item in a Vdir collection synchronously.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example std_store_item
//! ```

use io_vdir::{client::VdirClient, collection::Collection, item::ItemKind, path::VdirPath};
use tempfile::tempdir;

fn main() {
    let _ = env_logger::try_init();

    let tmp = tempdir().unwrap();
    let root = VdirPath::new(tmp.path().to_string_lossy().into_owned());

    let client = VdirClient::new(root.clone());

    let collection_path = root.join("contacts");
    let collection = Collection {
        path: collection_path.clone(),
        display_name: Some("Contacts".into()),
        description: None,
        color: Some("#3366ff".into()),
    };

    client.create_collection(collection).unwrap();

    let contents = b"BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Alice\r\nEND:VCARD\r\n".to_vec();

    let (id, path) = client
        .store_item(collection_path, None, ItemKind::Vcard, contents)
        .unwrap();

    println!("Stored item:");
    println!("  ID:   {id}");
    println!("  Path: {path}");
}
