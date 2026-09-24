use super::*;
use std::{
    collections::BTreeSet,
    io::{Read, Write},
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

pub(super) struct Scratch(pub PathBuf);
impl Scratch {
    pub fn new(parent: &Path) -> Result<Self> {
        let path = parent
            .canonicalize()?
            .join(format!(".den-portable-{}", ulid::Ulid::new()));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn empty_target(path: &Path) -> Result<()> {
    ensure!(
        path.file_name().is_some(),
        "Choose an unused data directory"
    );
    match fs::symlink_metadata(path) {
        Ok(meta) => ensure!(
            meta.file_type().is_dir() && fs::read_dir(path)?.next().is_none(),
            "Import target must be an empty directory, not a symlink"
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
    }
    Ok(())
}
fn upload_name(name: &str) -> bool {
    let (id, extension) = name.split_once('.').unwrap_or((name, ""));
    id.parse::<ulid::Ulid>().is_ok()
        && matches!(
            extension,
            "" | "part" | "recording" | "thumb.png" | "thumb.png.part"
        )
}
fn profile_name(name: &str) -> bool {
    name.split_once('/').is_some_and(|(id, file)| {
        id.parse::<ulid::Ulid>().is_ok()
            && matches!(file, "avatar" | "avatar.png" | "banner" | "banner.png")
    })
}
fn sound_name(name: &str) -> bool {
    name.split_once('/').is_some_and(|(owner, id)| {
        (owner == "server" || owner.parse::<ulid::Ulid>().is_ok()) && den_core::sound_id(id)
    })
}
/// `<user>` is the single file older servers kept; `<user>/<sha256>` and
/// `<user>/<sha256>.jpg` are a library image and its preview.
fn background_name(name: &str) -> bool {
    let hash = |id: &str| {
        id.len() == 64
            && id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    match name.split_once('/') {
        None => name.parse::<ulid::Ulid>().is_ok(),
        Some((user, file)) => {
            user.parse::<ulid::Ulid>().is_ok()
                && (hash(file) || file.strip_suffix(".jpg").is_some_and(hash))
        }
    }
}
fn archive_name(name: &str) -> bool {
    matches!(
        name,
        "den.db" | "manifest.json" | "uploads/" | "uploads/backgrounds/"
    ) || name.strip_prefix("uploads/sounds/").is_some_and(sound_name)
        || name
            .strip_prefix("uploads/profiles/")
            .is_some_and(profile_name)
        || name
            .strip_prefix("uploads/backgrounds/")
            .is_some_and(background_name)
        || name.strip_prefix("uploads/").is_some_and(upload_name)
}
fn private_file(path: &Path) -> Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}
fn copy_hash(reader: &mut impl Read, writer: &mut impl Write, limit: u64) -> Result<FileEntry> {
    let mut hash = Sha256::new();
    let mut size = 0;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        ensure!(
            size <= limit,
            "Archive entry exceeds its declared size or expansion limit"
        );
        hash.update(&buffer[..n]);
        writer.write_all(&buffer[..n])?;
    }
    Ok(FileEntry {
        size,
        sha256: hex(&hash.finalize()),
    })
}
pub(super) fn pack(
    snapshot: &Path,
    uploads: &Path,
    output: &Path,
    manifest: &mut Manifest,
) -> Result<()> {
    ensure!(
        fs::symlink_metadata(uploads)?.file_type().is_dir(),
        "Uploads must be a directory, not a symlink"
    );
    let mut paths = vec![("den.db".to_string(), snapshot.to_path_buf())];
    for entry in fs::read_dir(uploads)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("Non-UTF8 upload filename"))?;
        if name == "sounds" {
            ensure!(entry.file_type()?.is_dir(), "Sounds must be a directory");
            for owner in fs::read_dir(entry.path())? {
                let owner = owner?;
                ensure!(
                    owner.file_type()?.is_dir(),
                    "Sound owner must be a directory"
                );
                for file in fs::read_dir(owner.path())? {
                    let file = file?;
                    let name = format!(
                        "{}/{}",
                        owner.file_name().to_string_lossy(),
                        file.file_name().to_string_lossy()
                    );
                    if sound_name(&name) {
                        ensure!(file.file_type()?.is_file(), "Sound must be a regular file");
                        paths.push((format!("uploads/sounds/{name}"), file.path()));
                    }
                }
            }
        }
        if name == "profiles" {
            ensure!(
                entry.file_type()?.is_dir(),
                "Profiles must be a directory, not a symlink"
            );
            for user in fs::read_dir(entry.path())? {
                let user = user?;
                let id = user
                    .file_name()
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("Non-UTF8 profile filename"))?;
                if id.parse::<ulid::Ulid>().is_err() {
                    continue;
                }
                ensure!(
                    user.file_type()?.is_dir(),
                    "Profile must be a directory, not a symlink"
                );
                for file in fs::read_dir(user.path())? {
                    let file = file?;
                    let name = format!("{id}/{}", file.file_name().to_string_lossy());
                    if profile_name(&name) {
                        ensure!(
                            file.file_type()?.is_file(),
                            "Profile image is not a regular file"
                        );
                        paths.push((format!("uploads/profiles/{name}"), file.path()));
                    }
                }
            }
        }
        if name == "backgrounds" {
            ensure!(
                entry.file_type()?.is_dir(),
                "Backgrounds must be a directory, not a symlink"
            );
            for background in fs::read_dir(entry.path())? {
                let background = background?;
                let id = background
                    .file_name()
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("Non-UTF8 background filename"))?;
                if id.parse::<ulid::Ulid>().is_err() {
                    continue;
                }
                let kind = background.file_type()?;
                if kind.is_file() {
                    paths.push((format!("uploads/backgrounds/{id}"), background.path()));
                } else {
                    ensure!(kind.is_dir(), "Background library is not a directory");
                    for image in fs::read_dir(background.path())? {
                        let image = image?;
                        let file = image
                            .file_name()
                            .into_string()
                            .map_err(|_| anyhow::anyhow!("Non-UTF8 background filename"))?;
                        let name = format!("{id}/{file}");
                        if background_name(&name) {
                            ensure!(
                                image.file_type()?.is_file(),
                                "Background is not a regular file"
                            );
                            paths.push((format!("uploads/backgrounds/{name}"), image.path()));
                        }
                    }
                }
            }
        }
        // Only Den's flat upload names are data. Never traverse .ssh or copy host.toml/.env.
        if upload_name(&name) {
            ensure!(entry.file_type()?.is_file(), "Upload is not a regular file");
            paths.push((format!("uploads/{name}"), entry.path()));
        }
    }
    ensure!(paths.len() < MAX_FILES, "Too many archive files");
    paths.sort_by(|a, b| a.0.cmp(&b.0));
    let mut total = 0u64;
    for (_, p) in &paths {
        total = total
            .checked_add(fs::metadata(p)?.len())
            .context("Archive size overflow")?;
    }
    ensure!(total <= MAX_BYTES, "Archive exceeds 1 TiB expansion limit");
    ensure!(
        fs2::available_space(output.parent().unwrap())? > total + MAX_MANIFEST,
        "Not enough disk space for export"
    );
    let mut zip = ZipWriter::new(private_file(output)?);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o600);
    for (name, path) in paths {
        let mut file = fs::File::open(path)?;
        let size = file.metadata()?.len();
        zip.start_file(&name, options.large_file(size > u32::MAX as u64))?;
        let digest = copy_hash(&mut file, &mut zip, size)?;
        ensure!(digest.size == size, "Source file changed during export");
        manifest.files.insert(name, digest);
    }
    zip.start_file("manifest.json", options)?;
    let data = serde_json::to_vec_pretty(manifest)?;
    ensure!(data.len() as u64 <= MAX_MANIFEST, "Manifest too large");
    zip.write_all(&data)?;
    zip.finish()?.sync_all()?;
    Ok(())
}
pub(super) fn unpack(input: &Path, target: &Path) -> Result<Manifest> {
    let mut zip = ZipArchive::new(fs::File::open(input)?)?;
    ensure!(zip.len() <= MAX_FILES, "Too many archive entries");
    let mut names = BTreeSet::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let file = zip.by_index_raw(i)?;
        let name = std::str::from_utf8(file.name_raw()).context("Non-UTF8 ZIP path")?;
        ensure!(
            archive_name(name) && file.enclosed_name().is_some(),
            "Invalid archive path"
        );
        ensure!(names.insert(name.to_string()), "Duplicate archive path");
        let kind = file.unix_mode().unwrap_or(0) & 0o170000;
        ensure!(
            !file.is_symlink() && matches!(kind, 0 | 0o100000 | 0o040000),
            "Symlinks and special files are forbidden"
        );
        ensure!(
            file.is_dir() == matches!(name, "uploads/" | "uploads/backgrounds/"),
            "Invalid archive directory"
        );
        ensure!(
            name != "manifest.json" || file.size() <= MAX_MANIFEST,
            "Manifest too large"
        );
        total = total
            .checked_add(file.size())
            .context("Archive size overflow")?;
        ensure!(total <= MAX_BYTES, "Archive exceeds 1 TiB expansion limit");
    }
    let mut json = Vec::new();
    zip.by_name("manifest.json")?
        .take(MAX_MANIFEST + 1)
        .read_to_end(&mut json)?;
    ensure!(json.len() as u64 <= MAX_MANIFEST, "Manifest exceeds limit");
    let manifest: Manifest = serde_json::from_slice(&json)?;
    validate_schema(&manifest)?;
    names.remove("manifest.json");
    names.remove("uploads/");
    names.remove("uploads/backgrounds/");
    ensure!(
        names == manifest.files.keys().cloned().collect() && names.contains("den.db"),
        "Manifest file list does not match ZIP"
    );
    let db_size = manifest.files["den.db"].size;
    ensure!(
        fs2::available_space(target)?
            > total
                .saturating_add(db_size)
                .saturating_add(64 * 1024 * 1024),
        "Not enough disk space for import and migrations"
    );
    fs::create_dir(target.join("uploads"))?;
    fs::create_dir(target.join("uploads/backgrounds"))?;
    for (name, expected) in &manifest.files {
        let mut file = zip.by_name(name)?;
        ensure!(
            file.size() == expected.size && expected.sha256.len() == 64,
            "Manifest file size or hash is invalid"
        );
        if name.starts_with("uploads/profiles/")
            || name.starts_with("uploads/sounds/")
            || name.starts_with("uploads/backgrounds/")
        {
            fs::create_dir_all(target.join(name).parent().unwrap())?;
        }
        let mut output = private_file(&target.join(name))?;
        let actual = copy_hash(&mut file, &mut output, expected.size)?;
        ensure!(
            &actual == expected,
            "Archive file checksum or size mismatch"
        );
        output.sync_all()?;
    }
    Ok(manifest)
}
pub(super) fn sync_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
pub(super) fn sync_files(root: &Path) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            sync_files(&entry.path())?;
        } else {
            fs::File::open(entry.path())?.sync_all()?;
        }
    }
    sync_dir(root)
}
