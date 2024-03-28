use std::{
    ffi::CStr,
    io::{BufRead, Read, Write},
};

use anyhow::Context;

use crate::objects::{Kind, Object};

pub(crate) fn invoke(name_only: bool, tree_hash: &str) -> anyhow::Result<()> {
    let mut object = Object::read(&tree_hash).context("parse the object file")?;
    match object.kind {
        Kind::Tree => {
            let mut buf = Vec::new();
            let mut hash_buf = [0; 20];
            let stdout = std::io::stdout();
            let mut stdout = stdout.lock();
            loop {
                // tree entry: <mode> <name>\0<20_byte_sha>
                buf.clear();
                let n = object
                    .reader
                    .read_until(b'\0', &mut buf)
                    .context("read next tree entry")?;
                if n == 0 {
                    // end of file
                    break;
                }

                // read hash value
                object
                    .reader
                    .read_exact(&mut hash_buf)
                    .context("read 20 bytes hash value")?;

                // extract mode and name
                let mode_and_name =
                    CStr::from_bytes_with_nul(&buf).context("invalid tree entry")?;
                // since CStr doesn't necessary match UTF-8, to_str() can not be used here.
                let mut bits = mode_and_name.to_bytes().splitn(2, |&b| b == b' ');
                let mode = bits.next().expect("split always yields once");
                let mode = std::str::from_utf8(&mode).context("mode is always valid utf-8")?;
                let name = bits
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("tree entry has no file name"))?;

                // print
                if !name_only {
                    let kind = if mode == "40000" { "tree" } else { "blob" };
                    write!(stdout, "{:0>6} {} {}\t", mode, kind, hex::encode(&hash_buf))?;
                }

                stdout
                    .write_all(name)
                    .context("write tree entry to stdout")?;

                writeln!(stdout, "").context("write newline to stdout")?;
            }
        }
        _ => anyhow::bail!("don't know how to ls '{}' yet", object.kind),
    }

    Ok(())
}
