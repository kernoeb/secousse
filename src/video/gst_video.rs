use crate::video::error::Error;
use gst::message::MessageView;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_app::prelude::*;
use gstreamer_video as gst_video;
use parking_lot::{Mutex, RwLock};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) struct Frame(gst::Sample);

impl Frame {
    pub fn empty() -> Self {
        Self(gst::Sample::builder().build())
    }
}

/// Options for initializing a `Video` without post-construction locking.
#[derive(Debug, Clone)]
pub struct VideoOptions {
    /// Optional initial frame buffer capacity (0 disables buffering). Defaults to 3.
    pub frame_buffer_capacity: Option<usize>,
    /// Optional initial looping flag. Defaults to false.
    pub looping: Option<bool>,
    /// Optional initial playback speed. Defaults to 1.0.
    pub speed: Option<f64>,
}

pub fn warmup_gstreamer() {
    static WARMED: OnceLock<()> = OnceLock::new();
    if WARMED.set(()).is_err() {
        return;
    }

    std::thread::spawn(|| {
        if gst::init().is_err() {
            return;
        }

        if let Ok(pipeline) = gst::parse::launch("fakesrc num-buffers=1 ! fakesink")
            && let Ok(pipeline) = pipeline.downcast::<gst::Pipeline>()
        {
            let _ = pipeline.set_state(gst::State::Playing);
            let _ = pipeline.set_state(gst::State::Null);
        }
    });
}

impl Default for VideoOptions {
    fn default() -> Self {
        Self {
            frame_buffer_capacity: Some(3),
            looping: Some(false),
            speed: Some(1.0),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Internal {
    pub(crate) source: gst::Pipeline,
    pub(crate) alive: Arc<AtomicBool>,
    pub(crate) worker: Option<std::thread::JoinHandle<()>>,

    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) framerate: f64,
    // Stride information for NV12 format
    pub(crate) y_stride: i32,
    pub(crate) uv_stride: i32,

    pub(crate) frame: Arc<Mutex<Frame>>,
    pub(crate) upload_frame: Arc<AtomicBool>,
    pub(crate) is_eos: Arc<AtomicBool>,

    // Optional display size overrides. If only one is set, the other is
    // inferred using the natural aspect ratio (width / height).
    pub(crate) display_width_override: Option<u32>,
    pub(crate) display_height_override: Option<u32>,
}

impl Internal {
    pub(crate) fn set_paused(&mut self, paused: bool) {
        self.source
            .set_state(if paused {
                gst::State::Paused
            } else {
                gst::State::Playing
            })
            .unwrap(/* state was changed in ctor; state errors caught there */);
    }

    pub(crate) fn paused(&self) -> bool {
        self.source.state(gst::ClockTime::ZERO).1 == gst::State::Paused
    }
}

/// A multimedia video loaded from a URI (e.g., a local file path or HTTP stream).
#[derive(Debug, Clone)]
pub struct Video(pub(crate) Arc<RwLock<Internal>>);

impl Drop for Video {
    fn drop(&mut self) {
        // Only cleanup if this is the last reference
        if Arc::strong_count(&self.0) == 1
            && let Some(mut inner) = self.0.try_write()
        {
            inner
                .source
                .set_state(gst::State::Null)
                .expect("failed to set state");

            inner.alive.store(false, Ordering::SeqCst);
            if let Some(worker) = inner.worker.take()
                && let Err(err) = worker.join()
            {
                match err.downcast_ref::<String>() {
                    Some(e) => log::error!("Video thread panicked: {e}"),
                    None => log::error!("Video thread panicked with unknown reason"),
                }
            }
        }
    }
}

impl Video {
    /// Create a new video player from a given video which loads from `uri`,
    /// applying initialization options.
    pub fn new_with_options(uri: &url::Url, options: VideoOptions) -> Result<Self, Error> {
        gst::init()?;

        if crate::mock::enabled() {
            let pipeline = "videotestsrc is-live=true pattern=smpte ! videoscale ! videoconvert ! video/x-raw,format=NV12,width=1920,height=1080,framerate=60/1 ! appsink name=gpui_video drop=true max-buffers=3 enable-last-sample=false";
            let pipeline = gst::parse::launch(pipeline)?
                .downcast::<gst::Pipeline>()
                .map_err(|_| Error::Cast)?;
            let video_sink = pipeline
                .by_name("gpui_video")
                .ok_or(Error::Cast)?
                .downcast::<gst_app::AppSink>()
                .map_err(|_| Error::Cast)?;

            return Self::from_gst_pipeline_with_options(pipeline, video_sink, None, options);
        }

        let pipeline = format!(
            "playbin uri=\"{}\" buffer-size=10485760 buffer-duration=2000000000 video-sink=\"videoscale ! videoconvert ! appsink name=gpui_video drop=true max-buffers=3 enable-last-sample=false caps=video/x-raw,format=NV12,pixel-aspect-ratio=1/1\"",
            uri.as_str()
        );
        let pipeline = gst::parse::launch(pipeline.as_ref())?
            .downcast::<gst::Pipeline>()
            .map_err(|_| Error::Cast)?;

        let video_sink: gst::Element = pipeline.property("video-sink");
        let pad = video_sink.pads().first().cloned().unwrap();
        let pad = pad.dynamic_cast::<gst::GhostPad>().unwrap();
        let bin = pad
            .parent_element()
            .unwrap()
            .downcast::<gst::Bin>()
            .unwrap();
        let video_sink = bin.by_name("gpui_video").unwrap();
        let video_sink = video_sink.downcast::<gst_app::AppSink>().unwrap();

        Self::from_gst_pipeline_with_options(pipeline, video_sink, None, options)
    }

    /// Creates a new video based on an existing GStreamer pipeline and appsink,
    /// applying initialization options.
    pub fn from_gst_pipeline_with_options(
        pipeline: gst::Pipeline,
        video_sink: gst_app::AppSink,
        text_sink: Option<gst_app::AppSink>,
        options: VideoOptions,
    ) -> Result<Self, Error> {
        log::info!("[Video] Initializing GStreamer...");
        gst::init()?;
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        log::info!("[Video] Video ID: {}", id);

        macro_rules! cleanup {
            ($expr:expr) => {
                $expr.map_err(|e| {
                    let _ = pipeline.set_state(gst::State::Null);
                    e
                })
            };
        }

        let pad = video_sink.pads().first().cloned().unwrap();

        log::info!("[Video] Setting pipeline to Playing state...");
        cleanup!(pipeline.set_state(gst::State::Playing))?;

        // Avoid long blocking waits here; the worker will start pulling frames as they arrive.
        log::info!("[Video] Checking pipeline state (non-blocking)...");
        let state_result = pipeline.state(gst::ClockTime::from_mseconds(200));
        log::info!("[Video] Pipeline state result: {:?}", state_result);
        cleanup!(state_result.0)?;

        log::info!("[Video] Getting caps from pad...");
        let caps = pad.current_caps();

        let (width, height, framerate, y_stride, uv_stride) = if let Some(caps) = caps {
            log::info!("[Video] Got caps: {:?}", caps);
            let s = cleanup!(caps.structure(0).ok_or(Error::Caps))?;
            let width = cleanup!(s.get::<i32>("width").map_err(|_| Error::Caps))?;
            let height = cleanup!(s.get::<i32>("height").map_err(|_| Error::Caps))?;
            let framerate = cleanup!(s.get::<gst::Fraction>("framerate").map_err(|_| Error::Caps))?;
            let mut framerate = framerate.numer() as f64 / framerate.denom() as f64;

            // Obtain video info from caps for NV12 format (to get stride info)
            let vinfo = cleanup!(gst_video::VideoInfo::from_caps(&caps).map_err(|_| Error::Caps))?;
            let y_stride = vinfo.stride()[0]; // Y plane stride
            let uv_stride = vinfo.stride()[1]; // UV plane stride (for NV12)
            log::info!(
                "[Video] Strides - Y: {}, UV: {} (width: {})",
                y_stride,
                uv_stride,
                width
            );

            // For live streams (like HLS), framerate may be 0/1 (variable/unknown)
            // In this case, use a reasonable default framerate
            if framerate.is_nan()
                || framerate.is_infinite()
                || framerate < 0.0
                || framerate.abs() < f64::EPSILON
            {
                log::warn!(
                    "[Video] Framerate is {}, using default 30.0 fps for live stream",
                    framerate
                );
                framerate = 30.0; // Default to 30fps for live streams
            }
            log::info!("[Video] Using framerate: {} fps", framerate);
            (width, height, framerate, y_stride, uv_stride)
        } else {
            log::warn!("[Video] Caps not ready after startup, using defaults until first frame");
            (1, 1, 30.0, 1, 1)
        };

        let frame = Arc::new(Mutex::new(Frame::empty()));
        let upload_frame = Arc::new(AtomicBool::new(false));
        let frame_buffer = Arc::new(Mutex::new(VecDeque::new()));
        // Default to a small buffer so the element can consume buffered frames
        let frame_buffer_capacity = Arc::new(AtomicUsize::new(
            options.frame_buffer_capacity.unwrap_or_default(),
        ));
        let alive = Arc::new(AtomicBool::new(true));
        let last_frame_time = Arc::new(Mutex::new(Instant::now()));
        let initial_looping = options.looping.unwrap_or_default();
        let looping_flag = Arc::new(AtomicBool::new(initial_looping));
        let looping_ref = Arc::clone(&looping_flag);
        let initial_speed = options.speed.unwrap_or_default();
        let speed_state = Arc::new(AtomicU64::new(initial_speed.to_bits()));
        let speed_ref = Arc::clone(&speed_state);

        let frame_ref = Arc::clone(&frame);
        let upload_frame_ref = Arc::clone(&upload_frame);
        let frame_buffer_ref = Arc::clone(&frame_buffer);
        let frame_buffer_capacity_ref = Arc::clone(&frame_buffer_capacity);
        let alive_ref = Arc::clone(&alive);
        let last_frame_time_ref = Arc::clone(&last_frame_time);

        let subtitle_text = Arc::new(Mutex::new(None));
        let upload_text = Arc::new(AtomicBool::new(false));
        let subtitle_text_ref = Arc::clone(&subtitle_text);
        let upload_text_ref = Arc::clone(&upload_text);

        let pipeline_ref = pipeline.clone();
        let bus_ref = pipeline_ref.bus().unwrap();
        let is_eos = Arc::new(AtomicBool::new(false));
        let is_eos_ref = Arc::clone(&is_eos);

        let worker = std::thread::spawn(move || {
            let mut clear_subtitles_at = None;

            while alive_ref.load(Ordering::Acquire) {
                // Drain bus messages to detect EOS/errors
                while let Some(msg) = bus_ref.timed_pop(gst::ClockTime::from_seconds(0)) {
                    match msg.view() {
                        MessageView::Eos(_) => {
                            if looping_ref.load(Ordering::SeqCst) {
                                let mut flags = gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT;
                                let current_speed =
                                    f64::from_bits(speed_ref.load(Ordering::SeqCst));
                                if current_speed >= 0.0 {
                                    flags |= gst::SeekFlags::SNAP_AFTER;
                                } else {
                                    flags |= gst::SeekFlags::SNAP_BEFORE;
                                }
                                match pipeline_ref.seek(
                                    current_speed,
                                    flags,
                                    gst::SeekType::Set,
                                    gst::GenericFormattedValue::from(gst::ClockTime::from_seconds(
                                        0,
                                    )),
                                    gst::SeekType::None,
                                    gst::ClockTime::NONE,
                                ) {
                                    Ok(_) => {
                                        is_eos_ref.store(false, Ordering::SeqCst);
                                        let _ = pipeline_ref.set_state(gst::State::Playing);
                                        frame_buffer_ref.lock().clear();
                                        upload_frame_ref.store(false, Ordering::SeqCst);
                                        *subtitle_text_ref.lock() = None;
                                        upload_text_ref.store(true, Ordering::SeqCst);
                                        *last_frame_time_ref.lock() = Instant::now();
                                        continue;
                                    }
                                    Err(err) => {
                                        log::error!("failed to restart video for looping: {}", err);
                                        is_eos_ref.store(true, Ordering::SeqCst);
                                    }
                                }
                            } else {
                                is_eos_ref.store(true, Ordering::SeqCst);
                            }
                        }
                        MessageView::Error(err) => {
                            let debug = err.debug().unwrap_or_default();
                            log::error!(
                                "gstreamer error from {:?}: {} ({debug})",
                                err.src(),
                                err.error()
                            );
                        }
                        _ => {}
                    }
                }

                if is_eos_ref.load(Ordering::Acquire) {
                    // Stop busy-polling once EOS reached
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }
                if let Err(err) = (|| -> Result<(), gst::FlowError> {
                    // Try to pull a new sample; on timeout just continue (no frame this tick)
                    let maybe_sample =
                        if pipeline_ref.state(gst::ClockTime::ZERO).1 != gst::State::Playing {
                            video_sink.try_pull_preroll(gst::ClockTime::from_mseconds(16))
                        } else {
                            video_sink.try_pull_sample(gst::ClockTime::from_mseconds(16))
                        };

                    let Some(sample) = maybe_sample else {
                        // No sample available yet (timeout). Don't treat as error.
                        return Ok(());
                    };

                    *last_frame_time_ref.lock() = Instant::now();

                    let frame_segment = sample.segment().cloned().ok_or(gst::FlowError::Error)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                    let frame_pts = buffer.pts().ok_or(gst::FlowError::Error)?;
                    // For live streams, duration might not be available - use a reasonable default
                    let frame_duration = buffer
                        .duration()
                        .unwrap_or(gst::ClockTime::from_mseconds(33)); // ~30fps default

                    // Store the NV12 sample directly for GPU processing
                    {
                        let mut frame_guard = frame_ref.lock();
                        *frame_guard = Frame(sample);
                    }

                    // Push into frame buffer if enabled, trimming to capacity
                    let capacity = frame_buffer_capacity_ref.load(Ordering::SeqCst);
                    if capacity > 0 {
                        let sample_for_buffer = frame_ref.lock().0.clone();
                        let mut buf = frame_buffer_ref.lock();
                        buf.push_back(Frame(sample_for_buffer));
                        while buf.len() > capacity {
                            buf.pop_front();
                        }
                    }

                    // Always mark frame as ready for upload
                    upload_frame_ref.store(true, Ordering::SeqCst);

                    // Handle subtitles
                    if let Some(at) = clear_subtitles_at
                        && frame_pts >= at
                    {
                        *subtitle_text_ref.lock() = None;
                        upload_text_ref.store(true, Ordering::SeqCst);
                        clear_subtitles_at = None;
                    }

                    let text = text_sink
                        .as_ref()
                        .and_then(|sink| sink.try_pull_sample(gst::ClockTime::from_seconds(0)));
                    if let Some(text) = text {
                        let text_segment = text.segment().ok_or(gst::FlowError::Error)?;
                        let text = text.buffer().ok_or(gst::FlowError::Error)?;
                        let text_pts = text.pts().ok_or(gst::FlowError::Error)?;
                        let text_duration = text.duration().ok_or(gst::FlowError::Error)?;

                        let frame_running_time = frame_segment.to_running_time(frame_pts).value();
                        let frame_running_time_end = frame_segment
                            .to_running_time(frame_pts + frame_duration)
                            .value();

                        let text_running_time = text_segment.to_running_time(text_pts).value();
                        let text_running_time_end = text_segment
                            .to_running_time(text_pts + text_duration)
                            .value();

                        if text_running_time_end > frame_running_time
                            && frame_running_time_end > text_running_time
                        {
                            let duration = text.duration().unwrap_or(gst::ClockTime::ZERO);
                            let map = text.map_readable().map_err(|_| gst::FlowError::Error)?;

                            let text = std::str::from_utf8(map.as_slice())
                                .map_err(|_| gst::FlowError::Error)?
                                .to_string();
                            *subtitle_text_ref.lock() = Some(text);
                            upload_text_ref.store(true, Ordering::SeqCst);

                            clear_subtitles_at = Some(text_pts + duration);
                        }
                    }

                    Ok(())
                })() {
                    // Only log non-EOS errors
                    if err != gst::FlowError::Eos {
                        log::error!("error processing frame: {:?}", err);
                    }
                }
            }
        });

        // Apply initial playback speed if specified (must be after pipeline started)
        if (initial_speed - 1.0).abs() > f64::EPSILON {
            let position = cleanup!(
                pipeline
                    .query_position::<gst::ClockTime>()
                    .ok_or(Error::Caps)
            )?;
            if initial_speed > 0.0 {
                cleanup!(pipeline.seek(
                    initial_speed,
                    gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                    gst::SeekType::Set,
                    position,
                    gst::SeekType::End,
                    gst::ClockTime::from_seconds(0),
                ))?;
            } else {
                cleanup!(pipeline.seek(
                    initial_speed,
                    gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                    gst::SeekType::Set,
                    gst::ClockTime::from_seconds(0),
                    gst::SeekType::Set,
                    position,
                ))?;
            }
        }

        Ok(Video(Arc::new(RwLock::new(Internal {
            source: pipeline,
            alive,
            worker: Some(worker),

            width,
            height,
            framerate,
            y_stride,
            uv_stride,

            frame,
            upload_frame,
            is_eos,

            display_width_override: None,
            display_height_override: None,
        }))))
    }

    pub(crate) fn read(&'_ self) -> parking_lot::RwLockReadGuard<'_, Internal> {
        self.0.read()
    }

    pub(crate) fn write(&'_ self) -> parking_lot::RwLockWriteGuard<'_, Internal> {
        self.0.write()
    }

    /// Get the effective display size honoring overrides. If only one of
    /// width/height is overridden, the other is inferred from the natural
    /// aspect ratio, rounded to nearest pixel.
    pub fn display_size(&self) -> (u32, u32) {
        let inner = self.read();
        let natural_w = inner.width.max(0) as u32;
        let natural_h = inner.height.max(0) as u32;
        let ar = if natural_h == 0 {
            1.0
        } else {
            natural_w as f32 / natural_h as f32
        };

        match (inner.display_width_override, inner.display_height_override) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => {
                let h = if ar == 0.0 {
                    natural_h
                } else {
                    (w as f32 / ar).round() as u32
                };
                (w, h)
            }
            (None, Some(h)) => {
                let w = ((h as f32) * ar).round() as u32;
                (w, h)
            }
            (None, None) => (natural_w, natural_h),
        }
    }

    /// Set the volume multiplier of the audio.
    pub fn set_volume(&self, volume: f64) {
        {
            let inner = self.write();
            if inner.source.find_property("volume").is_some() {
                inner.source.set_property("volume", volume);
            }
        }
        let muted = self.muted();
        self.set_muted(muted);
    }

    /// Set if the audio is muted or not.
    pub fn set_muted(&self, muted: bool) {
        let inner = self.write();
        if inner.source.find_property("mute").is_some() {
            inner.source.set_property("mute", muted);
        }
    }

    /// Get if the audio is muted or not.
    pub fn muted(&self) -> bool {
        let inner = self.read();
        if inner.source.find_property("mute").is_some() {
            inner.source.property("mute")
        } else {
            false
        }
    }

    /// Get if the stream ended or not.
    pub fn eos(&self) -> bool {
        self.read().is_eos.load(Ordering::Acquire)
    }

    /// Set if the media is paused or not.
    pub fn set_paused(&self, paused: bool) {
        self.write().set_paused(paused)
    }

    /// Get if the media is paused or not.
    pub fn paused(&self) -> bool {
        self.read().paused()
    }

    /// Take the current GStreamer sample without copying.
    /// The caller gets ownership of the sample and can map its buffer in-place.
    /// Returns None if no frame is available.
    pub fn take_current_sample(&self) -> Option<gst::Sample> {
        let inner = self.read();
        let frame_guard = inner.frame.lock();
        // Check if there's a buffer in the sample
        if frame_guard.0.buffer().is_some() {
            Some(frame_guard.0.clone())
        } else {
            None
        }
    }

    /// Get frame metadata without copying any pixel data.
    /// Returns (width, height, y_stride, uv_stride).
    pub fn frame_meta(&self) -> (u32, u32, u32, u32) {
        let inner = self.read();
        (
            inner.width as u32,
            inner.height as u32,
            inner.y_stride as u32,
            inner.uv_stride as u32,
        )
    }

    /// Update cached metadata from caps if available.
    pub fn update_meta_from_caps(&self, caps: &gst::Caps) {
        let Some(s) = caps.structure(0) else {
            return;
        };

        let mut inner = self.write();
        if let Ok(width) = s.get::<i32>("width")
            && width > 0
        {
            inner.width = width;
        }
        if let Ok(height) = s.get::<i32>("height")
            && height > 0
        {
            inner.height = height;
        }

        if let Ok(framerate) = s.get::<gst::Fraction>("framerate") {
            let mut framerate = framerate.numer() as f64 / framerate.denom() as f64;
            if framerate.is_nan()
                || framerate.is_infinite()
                || framerate < 0.0
                || framerate.abs() < f64::EPSILON
            {
                framerate = 30.0;
            }
            inner.framerate = framerate;
        }

        if let Ok(vinfo) = gst_video::VideoInfo::from_caps(caps) {
            let strides = vinfo.stride();
            if strides.len() > 1 {
                inner.y_stride = strides[0];
                inner.uv_stride = strides[1];
            }
        }
    }

    /// Returns true if a new frame arrived since last check and resets the flag.
    pub fn take_frame_ready(&self) -> bool {
        self.read().upload_frame.swap(false, Ordering::SeqCst)
    }
}
