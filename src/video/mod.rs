//! Video player module
//!
//! Provides HLS video playback using GStreamer (code adapted from gpui-video-player).

mod element;
mod error;
mod gst_video;

pub use element::video;
pub use gst_video::{warmup_gstreamer, Video, VideoOptions};
