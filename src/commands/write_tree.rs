use crate::objects::{Kind, Object};
use anyhow::Context;
use std::cmp::Ordering;
use std::fs;
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn write_tree_for(path: &Path) -> anyhow::Result<Option<[u8; 20]>> {
    let mut dir =
        fs::read_dir(path).with_context(|| format!("open directory {}", path.display()))?;

    // sort the entries by file name
    let mut entries = Vec::new();
    while let Some(entry) = dir.next() {
        let entry = entry.with_context(|| format!("bad directory entry in {}", path.display()))?;
        let file_name = entry.file_name();
        let metadata = entry.metadata().with_context(|| {
            format!(
                "get the metadata of the directory entry. entry: {:?}",
                entry
            )
        })?;
        entries.push((entry, file_name, metadata));
    }
    entries.sort_unstable_by(|a, b| {
        // git has very specific rules for how to compare names
        // https://github.com/git/git/blob/c75fd8d8150afdf836b63a8e0534d9b9e3e111ba/tree.c#L99
        // (entry, file_name, metadata)
        let afn = a.1.as_encoded_bytes();
        let bfn = b.1.as_encoded_bytes();
        let common_len = std::cmp::min(afn.len(), bfn.len());
        match afn[..common_len].cmp(&bfn[..common_len]) {
            Ordering::Equal => {}
            o => return o,
        }
        if afn.len() == bfn.len() {
            return Ordering::Equal;
        }

        let c1 = if let Some(c) = afn.get(common_len).copied() {
            Some(c)
        } else if a.2.is_dir() {
            Some(b'/')
        } else {
            None
        };

        let c2 = if let Some(c) = bfn.get(common_len).copied() {
            Some(c)
        } else if b.2.is_dir() {
            Some(b'/')
        } else {
            None
        };

        c1.cmp(&c2)
    });

    // iterate throught the entries
    let mut tree_object_bytes: Vec<u8> = Vec::new();
    for (entry, file_name, metadata) in entries {
        // skip file
        if file_name == ".git" {
            continue;
        }

        if file_name == "target" {
            continue;
        }

        // get mode
        let mode = if metadata.is_dir() {
            "40000"
        } else if metadata.is_symlink() {
            "120000"
        } else if metadata.permissions().mode() & 0o111 != 0 {
            // has at least one executable bit set
            "100755"
        } else {
            "100644"
        };

        // get entry hash
        let path = entry.path();
        let hash = if metadata.is_dir() {
            let Some(hash) = write_tree_for(&path)? else {
                // empty directory, so don't include in parent
                continue;
            };
            hash
        } else {
            Object::blob_from_file(&path)?.write_to_object()?
        };

        // <mode> <name>\0<20_byte_sha>
        tree_object_bytes.extend(mode.as_bytes());
        tree_object_bytes.push(b' ');
        tree_object_bytes.extend(file_name.as_encoded_bytes());
        tree_object_bytes.push(b'\0');
        tree_object_bytes.extend(hash)
    }

    if tree_object_bytes.is_empty() {
        Ok(None)
    } else {
        let hash = Object {
            kind: Kind::Tree,
            expected_size: tree_object_bytes.len() as u64,
            reader: Cursor::new(tree_object_bytes),
        }
        .write_to_object()
        .context("write tree object")?;

        Ok(Some(hash))
    }
}

pub(crate) fn invoke() -> anyhow::Result<()> {
    let Some(hash) = write_tree_for(Path::new(".")).context("construct root tree object")? else {
        anyhow::bail!("asked to make tree object for empty tree")
    };

    println!("{}", hex::encode(hash));

    Ok(())
}
