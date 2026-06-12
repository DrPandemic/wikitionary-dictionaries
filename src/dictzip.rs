//! dictzip encoder.
//!
//! dictzip is a gzip variant that stays randomly seekable. The payload is a
//! single deflate stream, but the compressor issues a **full flush** at every
//! `CHUNK_LEN` bytes — flushing to a byte boundary and resetting the back-
//! reference window — so each chunk can be inflated on its own. The gzip header
//! carries an `RA` extra field listing every chunk's compressed length, letting
//! a reader map an uncompressed offset straight to the chunk(s) it needs.
//!
//! The result is both a valid plain gzip (one stream, only the final block sets
//! `BFINAL`) and the layout `stardict`'s `DictZip` reader expects.

use anyhow::{bail, Context, Result};
use flate2::{Compress, Compression, Crc, FlushCompress, Status};

/// Uncompressed bytes per chunk. The classic `dictzip` value; keeps each chunk's
/// compressed size well within the `u16` the `RA` table stores.
const CHUNK_LEN: usize = 58315;

/// Compress `data` into a dictzip (`.dict.dz`) byte stream.
pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
    let mut comp = Compress::new(Compression::best(), false); // raw deflate, no zlib wrapper
    let mut payload: Vec<u8> = Vec::with_capacity(data.len() / 2 + 64);
    let mut sizes: Vec<u16> = Vec::new();

    // At least one chunk, even for empty input, so the RA table is well-formed.
    let chunks: Vec<&[u8]> = if data.is_empty() {
        vec![&[][..]]
    } else {
        data.chunks(CHUNK_LEN).collect()
    };
    let last = chunks.len() - 1;
    for (i, chunk) in chunks.iter().enumerate() {
        let start = payload.len();
        feed(&mut comp, chunk, &mut payload)?;
        // Full-flush between chunks (resets the window → chunk stays standalone);
        // finish the deflate stream after the last one.
        let flush = if i == last { FlushCompress::Finish } else { FlushCompress::Full };
        run_flush(&mut comp, &mut payload, flush)?;

        let len = payload.len() - start;
        let len = u16::try_from(len)
            .with_context(|| format!("dictzip: chunk {i} compressed to {len} bytes (> u16)"))?;
        sizes.push(len);
    }

    if sizes.len() > u16::MAX as usize {
        bail!("dictzip: too many chunks ({}) for a u16 count", sizes.len());
    }

    // RA subfield payload: version, chunk length, chunk count, then the sizes.
    let ra_len = 6 + 2 * sizes.len();
    let xlen = 4 + ra_len; // "RA" + length field + payload

    let mut out = Vec::with_capacity(payload.len() + ra_len + 32);
    // gzip header: magic, deflate, FEXTRA flag, mtime=0, xfl=0, os=unknown.
    out.extend_from_slice(&[0x1f, 0x8b, 0x08, 0x04, 0, 0, 0, 0, 0, 0xff]);
    out.extend_from_slice(&(xlen as u16).to_le_bytes());
    out.extend_from_slice(b"RA");
    out.extend_from_slice(&(ra_len as u16).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // RA version
    out.extend_from_slice(&(CHUNK_LEN as u16).to_le_bytes());
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    for s in &sizes {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out.extend_from_slice(&payload);
    // gzip trailer: crc32 of the uncompressed data, then isize mod 2^32.
    let mut crc = Crc::new();
    crc.update(data);
    out.extend_from_slice(&crc.sum().to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());

    Ok(out)
}

/// Feed an entire chunk to the compressor without flushing.
fn feed(comp: &mut Compress, mut input: &[u8], out: &mut Vec<u8>) -> Result<()> {
    let mut scratch = [0u8; 16 * 1024];
    while !input.is_empty() {
        let in0 = comp.total_in();
        let out0 = comp.total_out();
        comp.compress(input, &mut scratch, FlushCompress::None)
            .context("deflate")?;
        let used = (comp.total_in() - in0) as usize;
        let produced = (comp.total_out() - out0) as usize;
        out.extend_from_slice(&scratch[..produced]);
        input = &input[used..];
    }
    Ok(())
}

/// Drive `flush` to completion. For `Finish` that means reaching `StreamEnd`;
/// for `Full` it means a call that does not fill the whole scratch buffer (zlib
/// keeps emitting sync blocks on every `Full` call, so "produced == 0" never
/// arrives — "didn't fill the buffer" is the real drained signal).
fn run_flush(comp: &mut Compress, out: &mut Vec<u8>, flush: FlushCompress) -> Result<()> {
    let mut scratch = [0u8; 16 * 1024];
    loop {
        let before = comp.total_out();
        let status = comp.compress(&[], &mut scratch, flush).context("deflate flush")?;
        let produced = (comp.total_out() - before) as usize;
        out.extend_from_slice(&scratch[..produced]);
        if matches!(status, Status::StreamEnd) || produced < scratch.len() {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    /// A proper dictzip is a valid single-stream gzip: a stock reader must
    /// reproduce the input byte-for-byte across many chunks.
    #[test]
    fn is_valid_gzip_roundtrip() {
        // > 3 chunks of mixed compressible / random-ish data.
        let data: Vec<u8> = (0..200_000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
        let dz = compress(&data).unwrap();
        let mut out = Vec::new();
        flate2::read::MultiGzDecoder::new(&dz[..])
            .read_to_end(&mut out)
            .unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn header_advertises_ra_field() {
        let dz = compress(b"hello dictzip").unwrap();
        assert_eq!(&dz[0..3], &[0x1f, 0x8b, 0x08]); // gzip + deflate
        assert_eq!(dz[3] & 0x04, 0x04); // FEXTRA set
        assert_eq!(&dz[12..14], b"RA"); // subfield id at start of extra
    }

    #[test]
    fn empty_input_is_well_formed() {
        let dz = compress(b"").unwrap();
        let mut out = Vec::new();
        flate2::read::MultiGzDecoder::new(&dz[..])
            .read_to_end(&mut out)
            .unwrap();
        assert!(out.is_empty());
    }
}
