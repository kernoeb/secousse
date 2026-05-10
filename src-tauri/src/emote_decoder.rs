// Decodes emote URLs (WebP, GIF, PNG, JPEG — static or animated) and returns
// raw RGBA frames as a single little-endian binary blob. See
// `src/components/EmoteImg.tsx` for why decoding lives here rather than in
// the webview.
//
// Wire format:
//   u32 width
//   u32 height
//   u32 frame_count
//   per frame: u32 duration_ms, RGBA bytes (width * height * 4)

use anyhow::{anyhow, Context, Result};
use image::{AnimationDecoder, ImageFormat};
use std::io::Cursor;

const MAX_DIMENSION: u32 = 256;
const MAX_FRAMES: usize = 200;
// Chat emotes render at 24 CSS px → 48 device px on 2x DPI screens.
// Capping native 56-128px sources to 48 px is pixel-perfect on Retina and
// still trims bitmap memory + IPC payload 50-90% on the larger sources.
const TARGET_MAX_DIM: u32 = 48;

#[tauri::command]
pub async fn decode_emote(
    state: tauri::State<'_, crate::AppState>,
    url: String,
) -> std::result::Result<tauri::ipc::Response, String> {
    decode_emote_inner(&state.http_client, &url)
        .await
        .map(tauri::ipc::Response::new)
        .map_err(|e| format!("{e:#}"))
}

async fn decode_emote_inner(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let bytes = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("fetch {url}"))?
        .error_for_status()?
        .bytes()
        .await?;

    let frames = match image::guess_format(&bytes) {
        Ok(ImageFormat::WebP) => decode_webp(&bytes)?,
        Ok(ImageFormat::Gif) => decode_gif(&bytes)?,
        _ => decode_static(&bytes)?,
    };
    if frames.is_empty() {
        return Err(anyhow!("no frames decoded"));
    }

    let (w, h) = (frames[0].0, frames[0].1);
    if w == 0 || h == 0 || w > MAX_DIMENSION || h > MAX_DIMENSION {
        return Err(anyhow!("dimensions {w}x{h} out of bounds"));
    }
    let frame_bytes = (w as usize) * (h as usize) * 4;

    let mut out = Vec::with_capacity(12 + frames.len() * (4 + frame_bytes));
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&(frames.len() as u32).to_le_bytes());
    for (fw, fh, dur, pixels) in &frames {
        if *fw != w || *fh != h || pixels.len() != frame_bytes {
            return Err(anyhow!("inconsistent frame size"));
        }
        out.extend_from_slice(&dur.to_le_bytes());
        out.extend_from_slice(pixels);
    }
    Ok(out)
}

type FrameTuple = (u32, u32, u32, Vec<u8>);

fn rgba_to_frame(img: image::DynamicImage) -> FrameTuple {
    let buf = downscale(img.to_rgba8());
    let (w, h) = buf.dimensions();
    (w, h, 0, buf.into_raw())
}

fn decode_webp(bytes: &[u8]) -> Result<Vec<FrameTuple>> {
    use image::codecs::webp::WebPDecoder;
    let decoder = WebPDecoder::new(Cursor::new(bytes))?;
    if decoder.has_animation() {
        collect_frames(decoder.into_frames())
    } else {
        // Reuse the already-constructed decoder instead of re-parsing.
        Ok(vec![rgba_to_frame(image::DynamicImage::from_decoder(decoder)?)])
    }
}

fn decode_gif(bytes: &[u8]) -> Result<Vec<FrameTuple>> {
    use image::codecs::gif::GifDecoder;
    let decoder = GifDecoder::new(Cursor::new(bytes))?;
    collect_frames(decoder.into_frames())
}

fn decode_static(bytes: &[u8]) -> Result<Vec<FrameTuple>> {
    Ok(vec![rgba_to_frame(image::load_from_memory(bytes)?)])
}

fn collect_frames(frames: image::Frames<'_>) -> Result<Vec<FrameTuple>> {
    let mut out = Vec::new();
    for (i, frame) in frames.enumerate() {
        if i >= MAX_FRAMES {
            break;
        }
        let frame = frame?;
        let (numer, denom) = frame.delay().numer_denom_ms();
        let duration_ms = if denom == 0 { 100 } else { (numer / denom).max(20) };
        let buf = downscale(frame.into_buffer());
        let (w, h) = buf.dimensions();
        out.push((w, h, duration_ms, buf.into_raw()));
    }
    Ok(out)
}

fn downscale(buf: image::RgbaImage) -> image::RgbaImage {
    let (w, h) = buf.dimensions();
    let max = w.max(h);
    if max <= TARGET_MAX_DIM {
        return buf;
    }
    let new_w = (w * TARGET_MAX_DIM) / max;
    let new_h = (h * TARGET_MAX_DIM) / max;
    image::imageops::resize(&buf, new_w, new_h, image::imageops::FilterType::Triangle)
}
