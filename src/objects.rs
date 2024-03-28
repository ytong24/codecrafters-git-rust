use anyhow::Context;
use core::fmt;
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use sha1::{Digest, Sha1};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Blob,
    Tree,
    Commit,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Blob => write!(f, "blob"),
            Kind::Tree => write!(f, "tree"),
            Kind::Commit => write!(f, "commit"),
        }
    }
}

pub(crate) struct Object<R> {
    pub(crate) kind: Kind,
    pub(crate) expected_size: u64,
    pub(crate) reader: R,
}

impl Object<()> {
    pub(crate) fn blob_from_file(file_path: impl AsRef<Path>) -> anyhow::Result<Object<impl Read>> {
        // get the file stat through metadata before read the content
        let file_path = file_path.as_ref();
        let stat =
            fs::metadata(&file_path).with_context(|| format!("stat {}", file_path.display()))?;

        let file = fs::File::open(&file_path).context("open the file to calculate SHA 1")?;

        Ok(Object {
            kind: Kind::Blob,
            expected_size: stat.len(),
            reader: file,
        })
    }

    pub(crate) fn read(object_hash: &str) -> anyhow::Result<Object<impl BufRead>> {
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
            "tree" => Kind::Tree,
            "commit" => Kind::Commit,
            _ => anyhow::bail!("we do not yet know how to print a '{kind}'"),
        };

        let size = size
            .parse::<u64>()
            .context(".git/objects file header has invalid size: {size}")?;

        let z = z.take(size);

        Ok(Object {
            kind,
            expected_size: size,
            reader: z,
        })
    }
}

impl<R> Object<R>
where
    R: Read,
{
    pub(crate) fn write(mut self, writer: impl Write) -> anyhow::Result<[u8; 20]> {
        // use Zlib encoder and Sha1 hasher to create BlobWriter
        let z = ZlibEncoder::new(writer, Compression::default());
        let mut writer = HashWriter {
            writer: z,
            hasher: Sha1::new(),
        };

        // encode header
        let header = format!("{} {}\0", self.kind, self.expected_size);
        write!(writer, "{}", &header)?;

        // encode content
        std::io::copy(&mut self.reader, &mut writer).context("stream file into blob")?; // don't need to read the file content into a buffer. io copy is enough

        let _ = writer
            .writer
            .finish()
            .context("compress header and file content")?;
        let hash_value = writer.hasher.finalize();

        Ok(hash_value.into())
    }

    pub(crate) fn write_to_object(self) -> anyhow::Result<[u8; 20]> {
        // compress the blob object and write to the file
        let tmp_file = "temp";
        let hash_value = self
            .write(fs::File::create(&tmp_file).context("create temporary file for blob")?)
            .context("write blob to file")?;

        let hash_hex = hex::encode(&hash_value);

        fs::create_dir_all(format!(".git/objects/{}", &hash_hex[..2]))
            .context("create subdir for .git/objects")?;
        fs::rename(
            &tmp_file,
            format!(".git/objects/{}/{}", &hash_hex[..2], &hash_hex[2..]),
        )
        .context("move temp blob file into .git/objects")?;

        Ok(hash_value)
    }
}

// encapsulate the compressed content writer and the hash writer into the same writer to avoid duplicate code
struct HashWriter<W> {
    writer: W,
    hasher: Sha1,
}

impl<W> Write for HashWriter<W>
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
