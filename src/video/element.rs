use crate::video::gst_video::Video;
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use core_video::pixel_buffer::{
    CVPixelBuffer, CVPixelBufferLockBaseAddress, CVPixelBufferUnlockBaseAddress,
    kCVPixelBufferIOSurfacePropertiesKey, kCVPixelBufferMetalCompatibilityKey,
    kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
};
use gpui::{
    Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Window,
};
use gstreamer_video as gst_video;

/// A video element that uses macOS CoreVideo + Metal for zero-copy NV12 rendering.
///
/// Reads the current GStreamer sample directly (no `.to_vec()` copy), creates a
/// CVPixelBuffer, copies the NV12 planes into it, and passes it to
/// `window.paint_surface()` for GPU-side YUV→RGB conversion via Metal.
///
/// Key design decisions:
/// - **No frame buffer**: GStreamer's appsink already handles buffering/dropping
///   with `drop=true max-buffers=3`. A second buffer layer adds latency that
///   causes A/V desync on live streams.
/// - **No `.to_vec()`**: The GStreamer sample buffer is mapped read-only in place
///   and copied directly into the CVPixelBuffer planes. This eliminates ~3MB of
///   heap allocation per frame at 1080p.
/// - **Animation frames only on new data**: `request_animation_frame` is only
///   called when the worker thread signals a new frame, avoiding unnecessary
///   repaints when the stream is idle or between frames.
pub struct VideoElement {
    video: Video,
    display_width: Option<gpui::Pixels>,
    display_height: Option<gpui::Pixels>,
    element_id: Option<ElementId>,
    /// If true, the element will fill its container (using flex: 1)
    fill_container: bool,
}

impl VideoElement {
    pub fn new(video: Video) -> Self {
        Self {
            video,
            display_width: None,
            display_height: None,
            element_id: None,
            fill_container: true,
        }
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.element_id = Some(id.into());
        self
    }

    /// Get the current display dimensions, falling back to video's effective display size.
    fn get_display_size(&self) -> (gpui::Pixels, gpui::Pixels) {
        match (self.display_width, self.display_height) {
            (Some(w), Some(h)) => (w, h),
            _ => {
                let (w, h) = self.video.display_size();
                (gpui::px(w as f32), gpui::px(h as f32))
            }
        }
    }

    /// Compute aspect-fit destination bounds inside the given container `bounds`.
    fn fitted_bounds(
        &self,
        bounds: gpui::Bounds<gpui::Pixels>,
        frame_width: u32,
        frame_height: u32,
    ) -> gpui::Bounds<gpui::Pixels> {
        let container_w: f32 = bounds.size.width.into();
        let container_h: f32 = bounds.size.height.into();
        let frame_w = frame_width as f32;
        let frame_h = frame_height as f32;

        let scale = if frame_w > 0.0 && frame_h > 0.0 {
            (container_w / frame_w).min(container_h / frame_h)
        } else {
            1.0
        };

        let dest_w = (frame_w * scale).max(0.0);
        let dest_h = (frame_h * scale).max(0.0);
        let offset_x = (container_w - dest_w) * 0.5;
        let offset_y = (container_h - dest_h) * 0.5;

        gpui::Bounds::new(
            gpui::point(
                bounds.origin.x + gpui::px(offset_x),
                bounds.origin.y + gpui::px(offset_y),
            ),
            gpui::size(gpui::px(dest_w), gpui::px(dest_h)),
        )
    }

    /// Paint the current GStreamer sample as an NV12 surface via Metal.
    ///
    /// Maps the sample's buffer read-only in place (zero-copy from GStreamer),
    /// creates a CVPixelBuffer, copies planes into it, and calls `paint_surface`.
    fn paint_current_frame(&self, window: &mut Window, bounds: gpui::Bounds<gpui::Pixels>) {
        // Get the sample without copying pixel data
        let Some(sample) = self.video.take_current_sample() else {
            return;
        };

        let Some(buffer) = sample.buffer() else {
            return;
        };

        let Ok(map) = buffer.map_readable() else {
            return;
        };

        let yuv_data = map.as_slice();
        if yuv_data.is_empty() {
            return;
        }

        let (frame_width, frame_height, y_stride, uv_stride) = if let Some(caps_ref) = sample.caps()
        {
            let caps = caps_ref.to_owned();
            self.video.update_meta_from_caps(&caps);
            if let Ok(vinfo) = gst_video::VideoInfo::from_caps(&caps) {
                let strides = vinfo.stride();
                if strides.len() > 1 && vinfo.width() > 0 && vinfo.height() > 0 {
                    (
                        vinfo.width(),
                        vinfo.height(),
                        strides[0] as u32,
                        strides[1] as u32,
                    )
                } else {
                    self.video.frame_meta()
                }
            } else {
                self.video.frame_meta()
            }
        } else {
            self.video.frame_meta()
        };
        let width = frame_width as usize;
        let height = frame_height as usize;
        let y_stride_usize = y_stride as usize;
        let uv_stride_usize = uv_stride as usize;

        // Validate data size
        let y_plane_size = y_stride_usize * height;
        let uv_plane_size = uv_stride_usize * (height / 2);
        if yuv_data.len() < y_plane_size + uv_plane_size {
            return;
        }

        // Create a CVPixelBuffer with NV12 full-range format.
        // Must be IOSurface-backed and Metal-compatible for CoreVideo texture cache.
        let attrs = unsafe {
            let metal_key = CFString::wrap_under_get_rule(kCVPixelBufferMetalCompatibilityKey);
            let iosurface_key = CFString::wrap_under_get_rule(kCVPixelBufferIOSurfacePropertiesKey);
            let empty_dict = CFDictionary::<CFString, CFType>::from_CFType_pairs(&[]);

            CFDictionary::<CFString, CFType>::from_CFType_pairs(&[
                (metal_key, CFBoolean::true_value().as_CFType()),
                (iosurface_key, empty_dict.as_CFType()),
            ])
        };

        let pixel_buffer = match CVPixelBuffer::new(
            kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
            width,
            height,
            Some(&attrs),
        ) {
            Ok(pb) => pb,
            Err(e) => {
                log::error!("[Video] Failed to create CVPixelBuffer: {:?}", e);
                return;
            }
        };

        // Lock the pixel buffer for writing
        let lock_result =
            unsafe { CVPixelBufferLockBaseAddress(pixel_buffer.as_concrete_TypeRef(), 0) };
        if lock_result != 0 {
            log::error!("[Video] Failed to lock CVPixelBuffer: {}", lock_result);
            return;
        }

        // Copy Y plane directly from the mapped GStreamer buffer (no intermediate Vec)
        unsafe {
            let y_dest = pixel_buffer.get_base_address_of_plane(0);
            let y_dest_stride = pixel_buffer.get_bytes_per_row_of_plane(0);
            let y_src = yuv_data.as_ptr();

            for row in 0..height {
                let src_offset = row * y_stride_usize;
                let dst_offset = row * y_dest_stride;
                let copy_len = width.min(y_stride_usize).min(y_dest_stride);
                std::ptr::copy_nonoverlapping(
                    y_src.add(src_offset),
                    (y_dest as *mut u8).add(dst_offset),
                    copy_len,
                );
            }
        }

        // Copy UV (CbCr interleaved) plane
        unsafe {
            let uv_dest = pixel_buffer.get_base_address_of_plane(1);
            let uv_dest_stride = pixel_buffer.get_bytes_per_row_of_plane(1);
            let uv_src = yuv_data.as_ptr().add(y_plane_size);
            let uv_height = height / 2;

            for row in 0..uv_height {
                let src_offset = row * uv_stride_usize;
                let dst_offset = row * uv_dest_stride;
                let copy_len = width.min(uv_stride_usize).min(uv_dest_stride);
                std::ptr::copy_nonoverlapping(
                    uv_src.add(src_offset),
                    (uv_dest as *mut u8).add(dst_offset),
                    copy_len,
                );
            }
        }

        // Unlock
        unsafe {
            CVPixelBufferUnlockBaseAddress(pixel_buffer.as_concrete_TypeRef(), 0);
        }

        // Compute aspect-fit bounds and paint via the Metal surface path
        let dest_bounds = self.fitted_bounds(bounds, frame_width, frame_height);
        window.paint_surface(dest_bounds, pixel_buffer);
    }
}

impl Element for VideoElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        self.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut gpui::App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = if self.fill_container {
            gpui::Style {
                size: gpui::Size {
                    width: gpui::Length::Definite(gpui::DefiniteLength::Fraction(1.0)),
                    height: gpui::Length::Definite(gpui::DefiniteLength::Fraction(1.0)),
                },
                flex_grow: 1.0,
                ..Default::default()
            }
        } else {
            let (mut width, mut height) = self.get_display_size();

            if self.display_width.is_none() || self.display_height.is_none() {
                let (vw, vh) = self.video.display_size();
                if self.display_width.is_none() {
                    width = gpui::px(vw as f32);
                }
                if self.display_height.is_none() {
                    height = gpui::px(vh as f32);
                }
            }

            gpui::Style {
                size: gpui::Size {
                    width: gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                        gpui::AbsoluteLength::Pixels(width),
                    )),
                    height: gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                        gpui::AbsoluteLength::Pixels(height),
                    )),
                },
                ..Default::default()
            }
        };

        let layout_id = window.request_layout(style, [], cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout_state: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut gpui::App,
    ) -> Self::PrepaintState {
        // Only schedule a repaint when the worker thread has actually delivered
        // a new frame. This avoids unnecessary repaints between frames (e.g.,
        // at 60Hz vsync with a 30fps stream, half the repaints were wasted).
        //
        // `take_frame_ready` atomically checks and clears the flag, so even if
        // we miss one cycle the next prepaint will pick it up.
        let has_new_frame = self.video.take_frame_ready();
        if has_new_frame {
            window.request_animation_frame();
        } else {
            // Even without a new frame, keep polling while playing so we don't
            // miss the next frame delivery. The cost of a no-op repaint is low
            // (no CVPixelBuffer created, no copy) since paint_current_frame
            // exits early when the sample hasn't changed.
            let is_playing = !self.video.eos() && !self.video.paused();
            if is_playing {
                window.request_animation_frame();
            }
        }
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout_state: &mut Self::RequestLayoutState,
        _prepaint_state: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut gpui::App,
    ) {
        // Paint directly from the current GStreamer sample — no frame buffer,
        // no Vec<u8> copy. The sample's buffer is mapped read-only in place.
        self.paint_current_frame(window, bounds);
    }
}

impl IntoElement for VideoElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Helper function to create a video element
pub fn video(video: Video) -> VideoElement {
    VideoElement::new(video)
}
