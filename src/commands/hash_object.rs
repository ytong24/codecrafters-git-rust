use anyhow::Context;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha1::{Digest, Sha1};
use std::fs;
use std::io::prelude::*;
use std::path::Path;

pub(crate) fn invoke(write_object: bool, file_path: &Path) -> anyhow::Result<()> {
    fn write_blob<W>(file_path: &Path, compressed_content_writer: W) -> anyhow::Result<String>
    where
        W: Write,
    {
        // get the file stat through metadata before read the content
        let stat =
            fs::metadata(&file_path).with_context(|| format!("stat {}", file_path.display()))?;

        // use Zlib encoder and Sha1 hasher to create BlobWriter
        let z = ZlibEncoder::new(compressed_content_writer, Compression::default());
        let mut writer = BlobWriter {
            writer: z,
            hasher: Sha1::new(),
        };

        // encode header
        let header = format!("blob {}\0", stat.len());
        write!(writer, "{}", &header)?;

        // encode content
        let mut file = fs::File::open(&file_path).context("open the file to calculate SHA 1")?;
        std::io::copy(&mut file, &mut writer).context("stream file into blob")?; // don't need to read the file content into a buffer. io copy is enough

        let _ = writer
            .writer
            .finish()
            .context("compress header and file content")?;
        let hash_value = writer.hasher.finalize();
        let hash_value = hex::encode(hash_value); // use hex::encode to change Vec<u8> to string

        Ok(hash_value)
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

// encapsulate the compressed content writer and the hash writer into the same writer to avoid duplicate code
struct BlobWriter<W> {
    writer: W,
    hasher: Sha1,
}

impl<W> Write for BlobWriter<W>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.write(buf)?;
        self.hasher.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}
