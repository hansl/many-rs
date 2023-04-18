use crate::storage::v1_forest;
use serde_json::Value;
use std::collections::HashMap;
use {
    crate::{
        migration::{InnerMigration, MIGRATIONS},
        storage::{InnerStorage, Operation},
    },
    core::mem::replace,
    linkme::distributed_slice,
    many_error::ManyError,
    merk_v1::rocksdb::IteratorMode,
    merk_v2::Op,
    std::path::{Path, PathBuf},
};

fn initialize(storage: &mut InnerStorage, extra: &HashMap<String, Value>) -> Result<(), ManyError> {
    let ledger_db_path = extra
        .get("ledger_db_path")
        .ok_or(ManyError::unknown("Missing ledger_db_path"))?;
    migrate_database(storage, ledger_db_path.as_str().unwrap())
}

fn migrate_database<P: AsRef<Path>>(storage: &mut InnerStorage, path: P) -> Result<(), ManyError> {
    let ledger_parent_path = path.as_ref().parent().unwrap();
    let new_storage_root = tempfile::tempdir_in(ledger_parent_path).map_err(ManyError::unknown)?;

    {
        // Drop storage, so we can open it again and migrate it.
        let _ = replace(
            storage,
            InnerStorage::V1(
                merk_v1::Merk::open(new_storage_root.path().join("unused_db"))
                    .map_err(ManyError::unknown)?,
            ),
        );
    }

    let new_storage_path = new_storage_root.path().join("new_ledger.db");

    {
        // Open old storage and new storage.
        let v1_storage = merk_v1::Merk::open(path.as_ref()).map_err(ManyError::unknown)?;
        let mut v2_storage = merk_v2::Merk::open(&new_storage_path).map_err(ManyError::unknown)?;

        // Migrate all keys from old storage to new storage.
        for pair in v1_forest(&v1_storage, IteratorMode::Start, Default::default()) {
            let (key, tree) = pair.map_err(ManyError::unknown)?;
            let value = tree.value().to_vec();
            v2_storage
                .apply(&[(key, Op::Put(value))])
                .map_err(ManyError::unknown)?;
        }
    }

    // Swap old storage directory and new storage directory.
    libxch::xch(path, new_storage_path).map_err(ManyError::unknown)?;

    // Open new storage inside the storage.
    {
        let _ = replace(
            storage,
            InnerStorage::open_v2(&path).map_err(ManyError::unknown)?,
        );
    }

    // Delete left over files.
    // If the plug is pulled here, all that remains is files inside a temporary
    // directory, you should clean that up manually. The integrity of the files
    // are not compromised otherwise.
    // The only issue here is that this migration would be run again, and that's
    // bad. The fix would be to enable the migration below the block height prior
    // to restarting many-ledger.
    std::fs::remove_dir_all(new_storage_root.path()).map_err(ManyError::unknown)?;

    Ok(())

    //
    // match storage {
    //     InnerStorage::V1(merk) => v1_forest(merk, IteratorMode::Start, Default::default())
    //         .map(|key_value_pair| {
    //             key_value_pair
    //                 .map(|(key, value)| (key, Operation::from(Op::Put(value.value().to_vec()))))
    //         })
    //         .collect::<Result<Vec<_>, _>>()
    //         .map_err(ManyError::unknown)
    //         .and_then(|trees| {
    //             InnerStorage::open_v2(["/tmp", "temp1"].iter().collect::<PathBuf>())
    //                 .map(|replacement| (trees, replacement))
    //                 .map_err(Into::into)
    //         })
    //         .and_then(|(trees, mut replacement)| {
    //             replacement
    //                 .apply(trees.as_slice())
    //                 .map_err(Into::into)
    //                 .map(|_| replacement)
    //         })
    //         .and_then(|mut replacement| {
    //             replacement
    //                 .commit(&[])
    //                 .map_err(Into::into)
    //                 .map(|_| replacement)
    //         })
    //         .and_then(|replacement| {
    //             merk_v1::Merk::open(["/tmp", "temp2"].iter().collect::<PathBuf>())
    //                 .map_err(ManyError::unknown)
    //                 .map(|new_storage| (new_storage, replacement))
    //         })
    //         .and_then(|(new_storage, replacement)| {
    //             replace(merk, new_storage)
    //                 .destroy()
    //                 .map_err(ManyError::unknown)
    //                 .map(|_| replacement)
    //         }),
    //     InnerStorage::V2(_) => {
    //         InnerStorage::open_v2(["/tmp", "temp1"].iter().collect::<PathBuf>()).map_err(Into::into)
    //     }
    // }
    // .and_then(|replacement| {
    //     InnerStorage::open_v2(path.as_ref())
    //         .map_err(ManyError::unknown)
    //         .map(|destination| (replacement, destination))
    // })
    // .and_then(|(replacement, mut destination)| match replacement {
    //     InnerStorage::V1(_) => {
    //         *storage = destination;
    //         Ok(())
    //     }
    //     InnerStorage::V2(ref merk) => v2_forest(merk, IteratorMode::Start, Default::default())
    //         .map(|key_value_pair| {
    //             key_value_pair
    //                 .map(|(key, value)| (key, Operation::from(Op::Put(value.value().to_vec()))))
    //         })
    //         .collect::<Result<Vec<_>, _>>()
    //         .map_err(ManyError::unknown)
    //         .and_then(|trees| destination.apply(trees.as_slice()).map_err(Into::into))
    //         .and_then(|_| {
    //             destination.commit(&[]).map_err(Into::into).map(|_| {
    //                 *storage = destination;
    //             })
    //         }),
    // })
}

#[distributed_slice(MIGRATIONS)]
pub static HASH_MIGRATION: InnerMigration<InnerStorage, ManyError, PathBuf> =
    InnerMigration::new_initialize(
        initialize,
        "Hash Migration",
        "Move data from old version of merk hash scheme to new version of merk hash scheme",
    );
