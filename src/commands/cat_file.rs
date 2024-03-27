use anyhow::Context;
use flate2::read::ZlibDecoder;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};

enum Kind {
    Blob,
}

pub(crate) fn invoke(pretty_print: bool, object_hash: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        pretty_print,
        "mode must be give without -p, and we don't support mode"
    );
    // read the file
    let f = fs::File::open(format!(
        ".git/objects/{}/{}",
        &object_hash[..2],
        &object_hash[2..]
    ))
    .context("Open in .git/objects")?;

    // decompress and parse the header
    let z = ZlibDecoder::new(f);
    let mut z = BufReader::new(z);
    let mut buf = Vec::new();
    z.read_until(0, &mut buf)
        .context("read header from .git/objects")?;

    let header = std::ffi::CStr::from_bytes_until_nul(&buf)
        .expect("know there is exactly one null, and it's at the end");
    let header = header
        .to_str()
        .context(".git/objects file header isn't valid UTF-8")?;

    let Some((kind, size)) = header.split_once(' ') else {
        anyhow::bail!(".git/objects file header did not start with a known type: '{header}'");
    };

    let kind = match kind {
        "blob" => Kind::Blob,
        _ => anyhow::bail!("we do not yet know how to print a '{kind}'"),
    };

    let size = size
        .parse::<usize>()
        .context(".git/objects file header has invalid size: {size}")?;

    // read the content
    buf.clear();
    buf.reserve_exact(size);
    buf.resize(size, 0);

    z.read_exact(&mut buf)
        .context("read true contents of .git/objects file")?;
    // check if the content size matches the size in header
    let n = z
        .read(&mut [0])
        .context("validate EOF in .git/objects file")?;
    anyhow::ensure!(n == 0, ".git/object file had {n} trailing bytes");

    // since the blob might be an image, println! cannot be used here because it only takes a string.
    // instead, use stdout directly
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    match kind {
        Kind::Blob => {
            stdout
                .write_all(&buf)
                .context("write object contents to stdout")?;
        }
    }

    Ok(())
}
