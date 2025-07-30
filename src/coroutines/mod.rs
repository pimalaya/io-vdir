//! Collection of I/O-free, resumable and composable Vdir state
//! machines.
//!
//! Coroutines emit [I/O] requests that need to be processed by
//! [runtimes] in order to continue their progression.
//!
//! [I/O]: crate::io
//! [runtimes]: crate::runtimes

#[path = "create-collection.rs"]
pub mod create_collection;
#[path = "create-item.rs"]
pub mod create_item;
#[path = "delete-collection.rs"]
pub mod delete_collection;
#[path = "delete-item.rs"]
pub mod delete_item;
#[path = "list-collections.rs"]
pub mod list_collections;
#[path = "list-items.rs"]
pub mod list_items;
#[path = "read-item.rs"]
pub mod read_item;
#[path = "update-collection.rs"]
pub mod update_collection;
#[path = "update-item.rs"]
pub mod update_item;
