use anyhow::{Context, Ok};
use clap::{Parser, Subcommand};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha1::{Digest, Sha1};
use std::fs;
use std::io::{prelude::*, BufReader};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

/// Doc comment
#[derive(Debug, Subcommand)]
enum Command {
    /// Doc comment
    Init,
    CatFile {
        #[clap(short = 'p')]
        pretty_print: bool,

        object_hash: String,
    },
    HashObject {
        #[clap(short = 'w')]
        write_object: bool,

        file_path: PathBuf,
    },
}

enum Kind {
    Blob,
}

fn main() -> anyhow::Result<()> {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    eprintln!("Logs from your program will appear here!");

    let args = Args::parse();

    match args.command {
        Command::Init => {
            fs::create_dir(".git").unwrap();
            fs::create_dir(".git/objects").unwrap();
            fs::create_dir(".git/refs").unwrap();
            fs::write(".git/HEAD", "ref: refs/heads/main\n").unwrap();
            println!("Initialized git directory")
        }
        Command::CatFile {
            pretty_print,
            object_hash,
        } => {
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
                anyhow::bail!(
                    ".git/objects file header did not start with a known type: '{header}'"
                );
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
        }
        Command::HashObject {
            write_object,
            file_path,
        } => {
            fn write_blob<W>(
                file_path: &Path,
                compressed_content_writer: W,
            ) -> anyhow::Result<String>
            where
                W: Write,
            {
                // get the file stat through metadata before read the content
                let stat = fs::metadata(&file_path)
                    .with_context(|| format!("stat {}", file_path.display()))?;

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
                let mut file =
                    fs::File::open(&file_path).context("open the file to calculate SHA 1")?;
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
        }
    }
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
