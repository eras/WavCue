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
}

fn read_wave(filename: &str) -> Result<WaveFileInfo, std::io::Error> {
    let file = File::open(filename)?;
    let mut reader = BufReader::new(file);
    let mut cues = Vec::new();
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
                if &buf_tag == b"fmt " {
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

    Ok(WaveFileInfo { header, cues })
}

fn process(filename: &str) -> Result<(), std::io::Error> {
    let wave = read_wave(filename)?;
    for cue in wave.cues {
        let sample_start = cue.sample_start;
        let seconds = sample_start as f64 / wave.header.sampling_rate as f64;
        println!("{:.3},Mark {}", seconds, cue.cue_id);
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
        eprintln!("usage: wav-cue filename.wav > filename.csv");
    }
}
