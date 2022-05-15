#[macro_use]
extern crate arrayref;

use std::env;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;

#[derive(Debug)]
enum DataChunkId {
    Data,
    Sint,
}

#[derive(Debug)]
struct CueEntry {
    cue_id: u32,
    position: u32,
    data_chunk_id: DataChunkId,
    chunk_start: u32,
    block_start: u32,
    sample_start: u32,
}

// bext: https://web.archive.org/web/20091229093941/http://tech.ebu.ch/docs/tech/tech3285.pdf page 7
// typedef struct broadcast_audio_extension {
//   CHAR Description[256]; /* ASCII : «Description of the sound sequence» */
//   CHAR Originator[32]; /* ASCII : «Name of the originator» */
//   CHAR OriginatorReference[32]; /* ASCII : «Reference of the originator» */
//   CHAR OriginationDate[10]; /* ASCII : «yyyy-mm-dd» */
//   CHAR OriginationTime[8]; /* ASCII : «hh-mm-ss» */
//   DWORD TimeReferenceLow; /* First sample count since midnight low word */
//   DWORD TimeReferenceHigh; /* First sample count since midnight, high word */
//   WORD Version; /* Version of the BWF; unsigned binary number */
//   BYTE UMID_0 /* Binary byte 0 of SMPTE UMID */
//   ....
//   BYTE UMID_63 /* Binary byte 63 of SMPTE UMID */
//   BYTE Reserved[190] ; /* 190 bytes, reserved for future use, set to “NULL” */
//   CHAR CodingHistory[]; /* ASCII : « History coding » */
// } BROADCAST_EXT
#[derive(Debug)]
struct BroadcastAudioExtension {
    description: String,          /* ASCII : «Description of the sound sequence» */
    originator: String,           /* ASCII : «Name of the originator» */
    originator_reference: String, /* ASCII : «Reference of the originator» */
    origination_date: String,     /* ASCII : «yyyy-mm-dd» */
    origination_time: String,     /* ASCII : «hh-mm-ss» */
    time_reference: u64,          /* First sample count since midnight */
    version: u16,                 /* Version of the BWF; unsigned binary number */
}

#[derive(Debug)]
struct Header {
    compression_code: u16,
    number_of_channels: u16,
    sampling_rate: u32,
    average_bytes_per_second: u32,
    block_align: u16,
    significant_bits_per_sample: u16,
    // number of extra format bytes	2	16
    //	extra format bytes	various	0x1A
}

struct WaveFileInfo {
    header: Header,
    cues: Vec<CueEntry>,
    bext: Option<BroadcastAudioExtension>,
}

fn read_wave(filename: &str) -> Result<WaveFileInfo, std::io::Error> {
    let file = File::open(filename)?;
    let mut reader = BufReader::new(file);
    let mut cues = Vec::new();
    let mut bext: Option<BroadcastAudioExtension> = None;
    let mut header: Option<Header> = None;

    let mut buf_riff: [u8; 4] = [0; 4];
    reader.read_exact(&mut buf_riff)?;

    // https://www.recordingblogs.com/wiki/format-chunk-of-a-wave-file
    if &buf_riff == b"RIFF" {
        let mut buf_size: [u8; 4] = [0; 4];
        reader.read_exact(&mut buf_size)?;
        let size = u32::from_le_bytes(buf_size);
        let mut bytes_processed = 0u32;
        eprintln!("Audio data size: {size}");
        // Read.
        let mut buf_wave: [u8; 4] = [0; 4];
        reader.read_exact(&mut buf_wave)?;
        if &buf_wave == b"WAVE" {
            let mut buf_tag: [u8; 4] = [0; 4];
            let mut buf_chunk32_size: [u8; 4] = [0; 4];
            // walk chunks
            while let Ok(()) = reader.read_exact(&mut buf_tag) {
                reader.read_exact(&mut buf_chunk32_size)?;
                let chunk_size = u32::from_le_bytes(buf_chunk32_size);
                assert!(chunk_size > 0); // TODO: use custom error type
                if &buf_tag == b"bext" {
                    let mut buf_bext: [u8; 348] = [0; 348];
                    assert!(chunk_size as usize >= buf_bext.len()); // TODO: use custom error type
                    reader.read_exact(&mut buf_bext)?;
                    reader.seek_relative(chunk_size as i64 - buf_bext.len() as i64)?;
                    let mut ofs = 0;
                    let description = String::from_utf8_lossy(array_ref!(buf_bext, ofs, 256))
                        .trim_end_matches(char::from(0))
                        .to_string();
                    ofs += 256;
                    let originator = String::from_utf8_lossy(array_ref!(buf_bext, ofs, 32))
                        .trim_end_matches(char::from(0))
                        .to_string();
                    ofs += 32;
                    let originator_reference =
                        String::from_utf8_lossy(array_ref!(buf_bext, ofs, 32))
                            .trim_end_matches(char::from(0))
                            .to_string();
                    ofs += 32;
                    let origination_date =
                        String::from_utf8_lossy(array_ref!(buf_bext, ofs, 10)).to_string();
                    ofs += 10;
                    let origination_time =
                        String::from_utf8_lossy(array_ref!(buf_bext, ofs, 8)).to_string();
                    ofs += 8;
                    let time_reference_low = u32::from_le_bytes(*array_ref!(buf_bext, ofs, 4));
                    ofs += 4;
                    let time_reference_high = u32::from_le_bytes(*array_ref!(buf_bext, ofs, 4));
                    ofs += 4;
                    let version = u16::from_le_bytes(*array_ref!(buf_bext, ofs, 2));
                    bext = Some(BroadcastAudioExtension {
                        description,
                        originator,
                        originator_reference,
                        origination_date,
                        origination_time,
                        time_reference: time_reference_low as u64
                            | ((time_reference_high as u64) << 32),
                        version,
                    });
                    eprintln!("{bext:?}");
                } else if &buf_tag == b"fmt " {
                    let mut buf_fmt: [u8; 16] = [0; 16];
                    assert!(chunk_size >= 16); // TODO: use custom error type
                    assert!(header.is_none()); // TODO: use custom error type
                    reader.read_exact(&mut buf_fmt)?;
                    reader.seek_relative(chunk_size as i64 - buf_fmt.len() as i64)?;
                    let compression_code = u16::from_le_bytes(*array_ref!(buf_fmt, 0, 2));
                    let number_of_channels = u16::from_le_bytes(*array_ref!(buf_fmt, 2, 2));
                    let sampling_rate = u32::from_le_bytes(*array_ref!(buf_fmt, 4, 4));
                    let average_bytes_per_second = u32::from_le_bytes(*array_ref!(buf_fmt, 8, 4));
                    let block_align = u16::from_le_bytes(*array_ref!(buf_fmt, 12, 2));
                    let significant_bits_per_sample =
                        u16::from_le_bytes(*array_ref!(buf_fmt, 14, 2));
                    header = Some(Header {
                        compression_code,
                        number_of_channels,
                        sampling_rate,
                        average_bytes_per_second,
                        block_align,
                        significant_bits_per_sample,
                    });
                    eprintln!("{header:?}");
                } else if &buf_tag == b"cue " {
                    // https://www.recordingblogs.com/wiki/cue-chunk-of-a-wave-file
                    let mut buf_num_cue_points: [u8; 4] = [0; 4];
                    reader.read_exact(&mut buf_num_cue_points)?;
                    let num_cue_points = u32::from_le_bytes(buf_num_cue_points);
                    assert!(chunk_size == 4 + 24 * num_cue_points); // TODO: use custom error type
                    for _ in 0..num_cue_points {
                        let mut buf_cue: [u8; 24] = [0; 24];
                        reader.read_exact(&mut buf_cue)?;

                        let cue_id = u32::from_le_bytes(*array_ref!(buf_cue, 0, 4));
                        let position = u32::from_le_bytes(*array_ref!(buf_cue, 4, 4));
                        let data_chunk_id = {
                            let id = array_ref!(buf_cue, 8, 4).clone();
                            if &id == b"data" {
                                DataChunkId::Data
                            } else if &id == b"sint" {
                                DataChunkId::Sint
                            } else {
                                // TODO: use custom error type
                                panic!("Aiee");
                            }
                        };

                        let chunk_start = u32::from_le_bytes(*array_ref!(buf_cue, 12, 4));

                        let block_start = u32::from_le_bytes(*array_ref!(buf_cue, 16, 4));

                        let sample_start = u32::from_le_bytes(*array_ref!(buf_cue, 20, 4));

                        let entry = CueEntry {
                            cue_id,
                            position,
                            data_chunk_id,
                            chunk_start,
                            block_start,
                            sample_start,
                        };

                        eprintln!("{entry:?}");

                        cues.push(entry);
                    }
                } else {
                    eprintln!("skipping {}", String::from_utf8_lossy(&buf_tag));
                    reader.seek_relative(chunk_size as i64)?;
                }
                bytes_processed += chunk_size as u32;
                // TODO: implement alingment per https://www.recordingblogs.com/wiki/format-chunk-of-a-wave-file
            }
            eprintln!("bytes left: {}", size as i64 - bytes_processed as i64);
        } else {
            eprintln!("Not a wav file");
        }
    } else {
        eprintln!("Not a wav file");
    }

    let header = match header {
        Some(header) => header,
        None => panic!("No header"), //TODO: use custom error type
    };

    Ok(WaveFileInfo { header, bext, cues })
}

fn process(filename: &str) -> Result<(), std::io::Error> {
    let wave = read_wave(filename)?;
    for cue in wave.cues {
        let sample_start = cue.sample_start;
        let seconds = sample_start as f64 / wave.header.sampling_rate as f64;
        let time_label = match wave.bext {
            None => String::from(""),
            Some(BroadcastAudioExtension { time_reference, .. }) => {
                let time = (time_reference as f64 + sample_start as f64)
                    / wave.header.sampling_rate as f64;
                let hour = (time / 3600f64) as u32;
                let min = (time / 60f64) as u32 % 60u32;
                let sec = time as u32 % 60u32;
                let time_fmt = format!(" {}:{:02}:{:02}", hour, min, sec);
                time_fmt
            }
        };

        println!("{:.3},Mark {}{}", seconds, cue.cue_id, time_label);
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        let filename = &args[1];
        if let Err(error) = process(filename) {
            eprintln!("Failed to process \"{filename}\": {error}");
        }
    } else {
        eprintln!("usage: zoom-cue filename.wav > filename.csv");
    }
}
