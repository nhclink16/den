use super::*;
use std::io::{Cursor, Read, Write};
use symphonia::core::{errors::Error as DecodeError, io::MediaSourceStream, probe::Hint};

pub const MAX_AUDIO: usize = 512 * 1024;
pub const MAX_PACK: usize = 12 * MAX_AUDIO;
pub fn mime(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"RIFF") {
        "audio/wav"
    } else if bytes.starts_with(b"OggS") {
        "audio/ogg"
    } else {
        "audio/mpeg"
    }
}
pub fn validate(bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_AUDIO {
        return Err(Error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "sound_size",
            "Sounds must be at most 512 KiB".into(),
        ));
    }
    let bad = |_| Error::bad("Sound must decode as WAV, MP3 or Ogg Vorbis");
    let source = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let mut format = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            source,
            &Default::default(),
            &Default::default(),
        )
        .map_err(bad)?
        .format;
    if format.tracks().len() != 1 {
        return Err(Error::bad("Sound must contain one audio stream"));
    }
    let track = format
        .default_track()
        .ok_or_else(|| Error::bad("No audio stream"))?;
    let rate = track.codec_params.sample_rate.unwrap_or(0);
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(0);
    if rate == 0 || rate > 192000 || channels == 0 || channels > 2 {
        return Err(Error::bad("Use mono or stereo audio, up to 192 kHz"));
    }
    if track
        .codec_params
        .n_frames
        .is_some_and(|n| n > u64::from(rate) * 5)
    {
        return Err(Error::bad("Sounds must be at most 5 seconds"));
    }
    let expected_frames = track.codec_params.n_frames;
    let mut frames = 0u64;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &Default::default())
        .map_err(bad)?;
    let mut seconds = 0.0;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(DecodeError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(bad(e)),
        };
        let decoded = decoder.decode(&packet).map_err(bad)?;
        frames += decoded.frames() as u64;
        seconds += decoded.frames() as f64 / f64::from(decoded.spec().rate);
        if seconds > 5.0 {
            return Err(Error::bad("Sounds must be at most 5 seconds"));
        }
    }
    if seconds == 0.0
        || decoder.finalize().verify_ok == Some(false)
        || (mime(bytes) == "audio/wav" && expected_frames.is_some_and(|n| n != frames))
    {
        return Err(Error::bad("Empty or damaged audio"));
    }
    Ok(())
}
pub struct Bundle {
    pub pack: SoundPack,
    pub files: std::collections::BTreeMap<String, Vec<u8>>,
}
pub fn unpack(bytes: Vec<u8>) -> Result<Bundle> {
    let bad = || Error::bad("Invalid sound pack ZIP");
    if bytes.len() > MAX_PACK {
        return Err(bad());
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| bad())?;
    if zip.len() > 12 {
        return Err(Error::bad(
            "Pack may contain pack.json and up to eleven audio files",
        ));
    }
    let mut files = std::collections::BTreeMap::new();
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|_| bad())?;
        let name = file.name().to_owned();
        let limit = if name == "pack.json" {
            16384
        } else {
            MAX_AUDIO
        };
        if file.is_dir()
            || file.is_symlink()
            || !matches!(file.unix_mode().unwrap_or(0) & 0o170000, 0 | 0o100000)
            || (name != "pack.json" && !sound_id(&name))
            || file.size() > limit as u64
            || files.contains_key(&name)
        {
            return Err(bad());
        }
        let mut data = Vec::new();
        (&mut file).take(limit as u64 + 1).read_to_end(&mut data)?;
        if data.len() > limit {
            return Err(bad());
        }
        if name != "pack.json" {
            validate(&data)?;
        }
        files.insert(name, data);
    }
    let pack: SoundPack =
        serde_json::from_slice(&files.remove("pack.json").ok_or_else(bad)?).map_err(|_| bad())?;
    pack.validate().map_err(Error::bad)?;
    for sound in pack.sounds.values() {
        if let SoundRef::Upload { id } = sound {
            if !files.contains_key(id) {
                return Err(Error::bad("Pack is missing an audio file"));
            }
        }
    }
    Ok(Bundle { pack, files })
}
pub fn pack(bundle: Bundle) -> Result<Vec<u8>> {
    let bad = |_| Error::bad("Could not export sound pack");
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("pack.json", options).map_err(bad)?;
    zip.write_all(
        &serde_json::to_vec_pretty(&bundle.pack).map_err(|_| Error::bad("Invalid pack"))?,
    )?;
    for (id, data) in bundle.files {
        zip.start_file(id, options).map_err(bad)?;
        zip.write_all(&data)?;
    }
    Ok(zip.finish().map_err(bad)?.into_inner())
}
