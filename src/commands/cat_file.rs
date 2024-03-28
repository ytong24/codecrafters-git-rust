use anyhow::Context;

use crate::objects::Object;

pub(crate) fn invoke(pretty_print: bool, object_hash: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        pretty_print,
        "mode must be give without -p, and we don't support mode"
    );
    let mut object = Object::read(&object_hash).context("parse the object file")?;

    match object.kind {
        crate::objects::Kind::Blob => {
            // since the blob might be an image, println! cannot be used here because it only takes a string.
            // instead, use stdout directly
            let stdout = std::io::stdout();
            let mut stdout = stdout.lock();
            let n = std::io::copy(&mut object.reader, &mut stdout)
                .context("write git object into stdout")?;
            anyhow::ensure!(
                n == object.expected_size,
                ".git/object file was not expected size (expected: {}, actual: {n})",
                object.expected_size
            );
        }
        _ => anyhow::bail!("don't know how to print '{}'", object.kind),
    }

    Ok(())
}
