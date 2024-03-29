use std::fmt::Write;
use std::io::Cursor;

use anyhow::Context;

use crate::objects::{Kind, Object};

pub(crate) fn invoke(
    parent_hash: Option<String>,
    message: &str,
    tree_hash: &str,
) -> anyhow::Result<()> {
    // the ?s will never be triggered since we are writing into a String
    let mut commit = String::new();

    writeln!(commit, "tree {}", tree_hash)?;
    if let Some(parent_hash) = parent_hash {
        writeln!(commit, "parent {}", parent_hash)?;
    }
    writeln!(
        commit,
        "author Yan Tong <132377526+ytong24@users.noreply.github.com> 1711657394 -0700"
    )?;
    writeln!(
        commit,
        "committer Yan Tong <132377526+ytong24@users.noreply.github.com> 1711657394 -0700"
    )?;

    writeln!(commit, "")?;
    writeln!(commit, "{message}")?;

    let hash_value = Object {
        kind: Kind::Commit,
        expected_size: commit.len() as u64,
        reader: Cursor::new(commit),
    }
    .write_to_object()
    .context("write commit object")?;

    println!("{}", hex::encode(hash_value));

    Ok(())
}
