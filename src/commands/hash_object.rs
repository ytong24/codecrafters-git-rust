use crate::objects::Object;
use anyhow::Context;
use std::fs;
use std::io::prelude::*;
use std::path::Path;

pub(crate) fn invoke(write_object: bool, file_path: &Path) -> anyhow::Result<()> {
    fn write_blob<W>(file_path: &Path, compressed_content_writer: W) -> anyhow::Result<String>
    where
        W: Write,
    {
        let hash_value = Object::blob_from_file(file_path)
            .context("open blob input file")?
            .write(compressed_content_writer)
            .context("stream file into blob")?;

        Ok(hex::encode(hash_value)) // use hex::encode to change Vec<u8> to string
    }

    let hash_value = if write_object {
        // if write_object is true, compress the blob object and write to the file
        let tmp_file = "temp";
        let hash_value = write_blob(
            &file_path,
            fs::File::create(&tmp_file).context("create temporary file for blob")?,
        )
        .context("write blob to file")?;

        fs::create_dir_all(format!(".git/objects/{}", &hash_value[..2]))
            .context("create subdir for .git/objects")?;
        fs::rename(
            &tmp_file,
            format!(".git/objects/{}/{}", &hash_value[..2], &hash_value[2..]),
        )
        .context("move temp blob file into .git/objects")?;

        hash_value
    } else {
        write_blob(&file_path, std::io::sink())?
    };

    println!("{hash_value}");

    Ok(())
}
