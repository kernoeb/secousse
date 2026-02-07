//! A minimal single-line text input component with blinking cursor and IME support.
//!
//! Inspired by gpui-component's input implementation (Apache-2.0).
//! Uses GPUI's `EntityInputHandler` for proper platform text input integration.
//!
//! ## Standard text field behaviour
//!
//! - **Unfocused**: cursor hidden, placeholder visible (if text is empty).
//! - **Focused**: blinking cursor visible, placeholder visible in muted colour
//!   until the user types something.
//! - **Keystroke**: cursor shown solid, blink resumes after a short pause.
//! - **Selection**: highlighted range, cursor at the active end.

use gpui::prelude::FluentBuilder;
use gpui::*;
use std::ops::Range;
use std::time::Duration;

use crate::theme;

// ── Blink cursor ──────────────────────────────────────────────────────

const BLINK_INTERVAL: Duration = Duration::from_millis(530);
const BLINK_PAUSE_DELAY: Duration = Duration::from_millis(600);
const CURSOR_WIDTH: Pixels = px(1.5);

/// A small entity that toggles cursor visibility on a timer.
pub struct BlinkCursor {
    visible: bool,
    epoch: usize,
    _task: Option<Task<()>>,
}

impl BlinkCursor {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            visible: false,
            epoch: 0,
            _task: None,
        }
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Start blinking. Cursor is immediately visible, then toggles every BLINK_INTERVAL.
    pub fn start(&mut self, cx: &mut Context<Self>) {
        self.visible = true;
        self.epoch += 1;
        let epoch = self.epoch;
        self._task = Some(cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                cx.background_executor().timer(BLINK_INTERVAL).await;
                let should_continue = cx
                    .update(|cx| {
                        this.update(cx, |this, cx| {
                            if this.epoch != epoch {
                                return false;
                            }
                            this.visible = !this.visible;
                            cx.notify();
                            true
                        })
                        .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if !should_continue {
                    break;
                }
            }
        }));
        cx.notify();
    }

    /// Stop blinking and hide the cursor.
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        self.epoch += 1;
        self.visible = false;
        self._task = None;
        cx.notify();
    }

    /// Pause blinking briefly (e.g. after a keystroke). Cursor stays solid/visible,
    /// then resumes blinking after BLINK_PAUSE_DELAY.
    pub fn pause(&mut self, cx: &mut Context<Self>) {
        self.visible = true;
        self.epoch += 1;
        let epoch = self.epoch;
        cx.notify();
        self._task = Some(cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            cx.background_executor().timer(BLINK_PAUSE_DELAY).await;
            let _ = cx.update(|cx| {
                let _ = this.update(cx, |this, cx| {
                    if this.epoch == epoch {
                        this.start(cx);
                    }
                });
            });
        }));
    }
}

// ── Actions ───────────────────────────────────────────────────────────

actions!(
    text_input,
    [
        Backspace,
        Delete,
        MoveLeft,
        MoveRight,
        MoveToStartOfLine,
        MoveToEndOfLine,
        MoveToPreviousWord,
        MoveToNextWord,
        SelectLeft,
        SelectRight,
        SelectAll,
        SelectToStartOfLine,
        SelectToEndOfLine,
        SelectToPreviousWord,
        SelectToNextWord,
        Copy,
        Cut,
        Paste,
        Enter,
        Escape,
        DeleteToPreviousWord,
        DeleteToEndOfLine,
    ]
);

/// Register keybindings for the text_input context. Call once at app startup.
pub fn init(cx: &mut App) {
    #[cfg(target_os = "macos")]
    let bindings = vec![
        KeyBinding::new("backspace", Backspace, Some("TextInput")),
        KeyBinding::new("delete", Delete, Some("TextInput")),
        KeyBinding::new("left", MoveLeft, Some("TextInput")),
        KeyBinding::new("right", MoveRight, Some("TextInput")),
        KeyBinding::new("cmd-left", MoveToStartOfLine, Some("TextInput")),
        KeyBinding::new("cmd-right", MoveToEndOfLine, Some("TextInput")),
        KeyBinding::new("alt-left", MoveToPreviousWord, Some("TextInput")),
        KeyBinding::new("alt-right", MoveToNextWord, Some("TextInput")),
        KeyBinding::new("shift-left", SelectLeft, Some("TextInput")),
        KeyBinding::new("shift-right", SelectRight, Some("TextInput")),
        KeyBinding::new("cmd-a", SelectAll, Some("TextInput")),
        KeyBinding::new("cmd-shift-left", SelectToStartOfLine, Some("TextInput")),
        KeyBinding::new("cmd-shift-right", SelectToEndOfLine, Some("TextInput")),
        KeyBinding::new("alt-shift-left", SelectToPreviousWord, Some("TextInput")),
        KeyBinding::new("alt-shift-right", SelectToNextWord, Some("TextInput")),
        KeyBinding::new("cmd-c", Copy, Some("TextInput")),
        KeyBinding::new("cmd-x", Cut, Some("TextInput")),
        KeyBinding::new("cmd-v", Paste, Some("TextInput")),
        KeyBinding::new("enter", Enter, Some("TextInput")),
        KeyBinding::new("escape", Escape, Some("TextInput")),
        KeyBinding::new("alt-backspace", DeleteToPreviousWord, Some("TextInput")),
        KeyBinding::new("cmd-backspace", DeleteToEndOfLine, Some("TextInput")),
        // Home/End
        KeyBinding::new("home", MoveToStartOfLine, Some("TextInput")),
        KeyBinding::new("end", MoveToEndOfLine, Some("TextInput")),
    ];

    #[cfg(not(target_os = "macos"))]
    let bindings = vec![
        KeyBinding::new("backspace", Backspace, Some("TextInput")),
        KeyBinding::new("delete", Delete, Some("TextInput")),
        KeyBinding::new("left", MoveLeft, Some("TextInput")),
        KeyBinding::new("right", MoveRight, Some("TextInput")),
        KeyBinding::new("home", MoveToStartOfLine, Some("TextInput")),
        KeyBinding::new("end", MoveToEndOfLine, Some("TextInput")),
        KeyBinding::new("ctrl-left", MoveToPreviousWord, Some("TextInput")),
        KeyBinding::new("ctrl-right", MoveToNextWord, Some("TextInput")),
        KeyBinding::new("shift-left", SelectLeft, Some("TextInput")),
        KeyBinding::new("shift-right", SelectRight, Some("TextInput")),
        KeyBinding::new("ctrl-a", SelectAll, Some("TextInput")),
        KeyBinding::new("shift-home", SelectToStartOfLine, Some("TextInput")),
        KeyBinding::new("shift-end", SelectToEndOfLine, Some("TextInput")),
        KeyBinding::new("ctrl-shift-left", SelectToPreviousWord, Some("TextInput")),
        KeyBinding::new("ctrl-shift-right", SelectToNextWord, Some("TextInput")),
        KeyBinding::new("ctrl-c", Copy, Some("TextInput")),
        KeyBinding::new("ctrl-x", Cut, Some("TextInput")),
        KeyBinding::new("ctrl-v", Paste, Some("TextInput")),
        KeyBinding::new("enter", Enter, Some("TextInput")),
        KeyBinding::new("escape", Escape, Some("TextInput")),
        KeyBinding::new("ctrl-backspace", DeleteToPreviousWord, Some("TextInput")),
    ];

    cx.bind_keys(bindings);
}

// ── Events ────────────────────────────────────────────────────────────

/// Events emitted by the text input.
#[derive(Clone, Debug)]
pub enum TextInputEvent {
    /// The text content changed.
    Change(String),
    /// The user pressed Enter.
    Submit(String),
    /// The user pressed Escape.
    Escape,
}

// ── TextInputState ────────────────────────────────────────────────────

/// The state entity for a single-line text input.
pub struct TextInputState {
    pub focus_handle: FocusHandle,
    text: String,
    /// Byte offset range of current selection / cursor.
    selected_range: Range<usize>,
    /// If true, cursor is at start of selection; otherwise at end.
    selection_reversed: bool,
    /// Temporary IME composition range (byte offsets).
    ime_marked_range: Option<Range<usize>>,
    blink_cursor: Entity<BlinkCursor>,
    /// Cached shaped line from last paint.
    last_shaped_line: Option<ShapedLine>,
    /// Cached element bounds from last paint.
    last_bounds: Option<Bounds<Pixels>>,
    /// Horizontal scroll offset for overflowing text.
    scroll_offset: Pixels,
    /// Track previous focus state to detect focus transitions.
    was_focused: bool,
}

impl EventEmitter<TextInputEvent> for TextInputState {}

impl TextInputState {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let blink_cursor = cx.new(|cx| BlinkCursor::new(cx));

        // Repaint this entity whenever the blink cursor toggles.
        cx.observe(&blink_cursor, |_, _, cx| cx.notify()).detach();

        Self {
            focus_handle,
            text: String::new(),
            selected_range: 0..0,
            selection_reversed: false,
            ime_marked_range: None,
            blink_cursor,
            last_shaped_line: None,
            last_bounds: None,
            scroll_offset: px(0.0),
            was_focused: false,
        }
    }

    /// Detect focus transitions and start / stop the blink timer.
    /// Called once per render from `TextInput::render`.
    fn update_focus_blink(&mut self, is_focused: bool, cx: &mut Context<Self>) {
        if is_focused && !self.was_focused {
            // Gained focus → start blinking.
            self.blink_cursor.update(cx, |b, cx| b.start(cx));
        } else if !is_focused && self.was_focused {
            // Lost focus → stop blinking, hide cursor.
            self.blink_cursor.update(cx, |b, cx| b.stop(cx));
        }
        self.was_focused = is_focused;
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.text = text.into();
        self.selected_range = self.text.len()..self.text.len();
        cx.notify();
    }

    #[allow(dead_code)]
    pub fn focus(&self, window: &mut Window) {
        window.focus(&self.focus_handle);
    }

    #[allow(dead_code)]
    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    fn cursor(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn has_selection(&self) -> bool {
        self.selected_range.start != self.selected_range.end
    }

    fn selected_text(&self) -> &str {
        &self.text[self.selected_range.clone()]
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = offset.min(self.text.len());
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.pause_blink(cx);
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = offset.min(self.text.len());
        if self.selection_reversed {
            self.selected_range.start = offset;
            if self.selected_range.start > self.selected_range.end {
                self.selection_reversed = false;
                std::mem::swap(
                    &mut self.selected_range.start,
                    &mut self.selected_range.end,
                );
            }
        } else {
            self.selected_range.end = offset;
            if self.selected_range.end < self.selected_range.start {
                self.selection_reversed = true;
                std::mem::swap(
                    &mut self.selected_range.start,
                    &mut self.selected_range.end,
                );
            }
        }
        self.pause_blink(cx);
        cx.notify();
    }

    fn delete_range(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        if range.start >= range.end || range.start > self.text.len() {
            return;
        }
        let range = range.start..range.end.min(self.text.len());
        self.text.replace_range(range.clone(), "");
        self.selected_range = range.start..range.start;
        self.selection_reversed = false;
        self.pause_blink(cx);
        cx.emit(TextInputEvent::Change(self.text.clone()));
        cx.notify();
    }

    fn insert_text(&mut self, text: &str, cx: &mut Context<Self>) {
        // Replace selection (or insert at cursor)
        let range = self.selected_range.clone();
        self.text.replace_range(range.clone(), text);
        let new_cursor = range.start + text.len();
        self.selected_range = new_cursor..new_cursor;
        self.selection_reversed = false;
        self.ime_marked_range = None;
        self.pause_blink(cx);
        cx.emit(TextInputEvent::Change(self.text.clone()));
        cx.notify();
    }

    fn pause_blink(&self, cx: &mut Context<Self>) {
        self.blink_cursor.update(cx, |b, cx| b.pause(cx));
    }

    fn previous_word_boundary(&self, offset: usize) -> usize {
        let text = &self.text[..offset];
        let trimmed = text.trim_end();
        if trimmed.is_empty() {
            return 0;
        }
        trimmed
            .rfind(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
            .map(|i| i + 1)
            .unwrap_or(0)
    }

    fn next_word_boundary(&self, offset: usize) -> usize {
        let text = &self.text[offset..];
        let first_space = text
            .find(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
            .unwrap_or(text.len());
        let after = &text[first_space..];
        let first_non_space = after
            .find(|c: char| !c.is_whitespace() && !c.is_ascii_punctuation())
            .unwrap_or(after.len());
        offset + first_space + first_non_space
    }

    // ── Action handlers ───────────────────────────────────────────────

    fn action_backspace(&mut self, _: &Backspace, _window: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            self.delete_range(self.selected_range.clone(), cx);
        } else if self.cursor() > 0 {
            let prev = self.prev_char_boundary(self.cursor());
            self.delete_range(prev..self.cursor(), cx);
        }
    }

    fn action_delete(&mut self, _: &Delete, _window: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            self.delete_range(self.selected_range.clone(), cx);
        } else if self.cursor() < self.text.len() {
            let next = self.next_char_boundary(self.cursor());
            self.delete_range(self.cursor()..next, cx);
        }
    }

    fn action_delete_prev_word(
        &mut self,
        _: &DeleteToPreviousWord,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.has_selection() {
            self.delete_range(self.selected_range.clone(), cx);
        } else {
            let boundary = self.previous_word_boundary(self.cursor());
            self.delete_range(boundary..self.cursor(), cx);
        }
    }

    fn action_delete_to_end(
        &mut self,
        _: &DeleteToEndOfLine,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.has_selection() {
            self.delete_range(self.selected_range.clone(), cx);
        } else {
            self.delete_range(self.cursor()..self.text.len(), cx);
        }
    }

    fn action_move_left(&mut self, _: &MoveLeft, _window: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            self.move_to(self.selected_range.start, cx);
        } else if self.cursor() > 0 {
            let prev = self.prev_char_boundary(self.cursor());
            self.move_to(prev, cx);
        }
    }

    fn action_move_right(&mut self, _: &MoveRight, _window: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            self.move_to(self.selected_range.end, cx);
        } else if self.cursor() < self.text.len() {
            let next = self.next_char_boundary(self.cursor());
            self.move_to(next, cx);
        }
    }

    fn action_move_home(
        &mut self,
        _: &MoveToStartOfLine,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to(0, cx);
    }

    fn action_move_end(
        &mut self,
        _: &MoveToEndOfLine,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_to(self.text.len(), cx);
    }

    fn action_move_prev_word(
        &mut self,
        _: &MoveToPreviousWord,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let boundary = self.previous_word_boundary(self.cursor());
        self.move_to(boundary, cx);
    }

    fn action_move_next_word(
        &mut self,
        _: &MoveToNextWord,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let boundary = self.next_word_boundary(self.cursor());
        self.move_to(boundary, cx);
    }

    fn action_select_left(
        &mut self,
        _: &SelectLeft,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cursor = self.cursor();
        if cursor > 0 {
            let prev = self.prev_char_boundary(cursor);
            self.select_to(prev, cx);
        }
    }

    fn action_select_right(
        &mut self,
        _: &SelectRight,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cursor = self.cursor();
        if cursor < self.text.len() {
            let next = self.next_char_boundary(cursor);
            self.select_to(next, cx);
        }
    }

    fn action_select_all(
        &mut self,
        _: &SelectAll,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_range = 0..self.text.len();
        self.selection_reversed = false;
        self.pause_blink(cx);
        cx.notify();
    }

    fn action_select_to_start(
        &mut self,
        _: &SelectToStartOfLine,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(0, cx);
    }

    fn action_select_to_end(
        &mut self,
        _: &SelectToEndOfLine,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.text.len(), cx);
    }

    fn action_select_prev_word(
        &mut self,
        _: &SelectToPreviousWord,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let boundary = self.previous_word_boundary(self.cursor());
        self.select_to(boundary, cx);
    }

    fn action_select_next_word(
        &mut self,
        _: &SelectToNextWord,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let boundary = self.next_word_boundary(self.cursor());
        self.select_to(boundary, cx);
    }

    fn action_copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.selected_text().to_string()));
        }
    }

    fn action_cut(&mut self, _: &Cut, _window: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.selected_text().to_string()));
            self.delete_range(self.selected_range.clone(), cx);
        }
    }

    fn action_paste(&mut self, _: &Paste, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(item) = cx.read_from_clipboard() {
            let text = item.text().unwrap_or_default();
            // Single line: take only first line
            let line = text.lines().next().unwrap_or("");
            let clean: String = line.chars().filter(|c| !c.is_control()).collect();
            self.insert_text(&clean, cx);
        }
    }

    fn action_enter(&mut self, _: &Enter, _window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextInputEvent::Submit(self.text.clone()));
    }

    fn action_escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextInputEvent::Escape);
        window.blur();
    }

    // ── Char boundary helpers ─────────────────────────────────────────

    fn prev_char_boundary(&self, offset: usize) -> usize {
        if offset == 0 {
            return 0;
        }
        let mut i = offset - 1;
        while i > 0 && !self.text.is_char_boundary(i) {
            i -= 1;
        }
        i
    }

    fn next_char_boundary(&self, offset: usize) -> usize {
        if offset >= self.text.len() {
            return self.text.len();
        }
        let mut i = offset + 1;
        while i < self.text.len() && !self.text.is_char_boundary(i) {
            i += 1;
        }
        i
    }

    // ── UTF-16 conversion helpers ─────────────────────────────────────

    fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text[..offset].encode_utf16().count()
    }

    fn offset_from_utf16(&self, utf16_offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.text.chars() {
            if utf16_count >= utf16_offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }
}

// ── EntityInputHandler ────────────────────────────────────────────────

impl EntityInputHandler for TextInputState {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let start = self.offset_from_utf16(range_utf16.start);
        let end = self.offset_from_utf16(range_utf16.end);
        Some(self.text[start..end.min(self.text.len())].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let start = self.offset_to_utf16(self.selected_range.start);
        let end = self.offset_to_utf16(self.selected_range.end);
        Some(UTF16Selection {
            range: start..end,
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.ime_marked_range.as_ref().map(|r| {
            let start = self.offset_to_utf16(r.start);
            let end = self.offset_to_utf16(r.end);
            start..end
        })
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.ime_marked_range = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = if let Some(r) = range_utf16 {
            let start = self.offset_from_utf16(r.start);
            let end = self.offset_from_utf16(r.end);
            start..end
        } else if let Some(ime_range) = self.ime_marked_range.take() {
            ime_range
        } else {
            self.selected_range.clone()
        };

        let range = range.start.min(self.text.len())..range.end.min(self.text.len());
        self.text.replace_range(range.clone(), text);
        let new_cursor = range.start + text.len();
        self.selected_range = new_cursor..new_cursor;
        self.selection_reversed = false;
        self.ime_marked_range = None;
        self.pause_blink(cx);
        cx.emit(TextInputEvent::Change(self.text.clone()));
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = if let Some(r) = range_utf16 {
            let start = self.offset_from_utf16(r.start);
            let end = self.offset_from_utf16(r.end);
            start..end
        } else if let Some(ime_range) = self.ime_marked_range.take() {
            ime_range
        } else {
            self.selected_range.clone()
        };

        let range = range.start.min(self.text.len())..range.end.min(self.text.len());
        self.text.replace_range(range.clone(), new_text);
        let mark_start = range.start;
        let mark_end = range.start + new_text.len();
        self.ime_marked_range = Some(mark_start..mark_end);

        if let Some(sel_utf16) = new_selected_range_utf16 {
            let abs_start = mark_start + self.offset_from_utf16(sel_utf16.start);
            let abs_end = mark_start + self.offset_from_utf16(sel_utf16.end);
            self.selected_range = abs_start..abs_end;
        } else {
            self.selected_range = mark_end..mark_end;
        }

        self.pause_blink(cx);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let start = self.offset_from_utf16(range_utf16.start);
        if let Some(shaped) = &self.last_shaped_line {
            let x = shaped.x_for_index(start);
            Some(Bounds::new(
                point(
                    element_bounds.origin.x + x - self.scroll_offset,
                    element_bounds.origin.y,
                ),
                size(px(1.0), element_bounds.size.height),
            ))
        } else {
            Some(element_bounds)
        }
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        if let Some(shaped) = &self.last_shaped_line {
            let local_x = point.x + self.scroll_offset;
            let idx = shaped.closest_index_for_x(local_x);
            Some(self.offset_to_utf16(idx))
        } else {
            None
        }
    }
}

// ── Render (so Entity<TextInputState> can be used as a child) ─────────

impl Render for TextInputState {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        TextInputElement {
            state: _cx.entity().clone(),
        }
    }
}

// ── TextInputElement ──────────────────────────────────────────────────

struct TextInputElement {
    state: Entity<TextInputState>,
}

impl IntoElement for TextInputElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

struct TextInputPrepaintState {
    line: Option<ShapedLine>,
    cursor_pos: Option<Pixels>,
    selection_rect: Option<Bounds<Pixels>>,
}

impl Element for TextInputElement {
    type RequestLayoutState = ();
    type PrepaintState = TextInputPrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let style = Style {
            size: Size {
                width: Length::Definite(DefiniteLength::Fraction(1.0)),
                height: Length::Definite(DefiniteLength::Absolute(AbsoluteLength::Pixels(
                    window.line_height(),
                ))),
            },
            ..Default::default()
        };
        let layout_id = window.request_layout(style, [], cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout_state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> TextInputPrepaintState {
        let (text_owned, text_empty, cursor_byte, has_sel, sel_range, scroll_offset) = {
            let state = self.state.read(cx);
            (
                state.text.clone(),
                state.text.is_empty(),
                state.cursor(),
                state.has_selection(),
                state.selected_range.clone(),
                state.scroll_offset,
            )
        };

        let font_size = px(13.0);
        let text_style = window.text_style();
        let font = text_style.font();
        let text_color: Hsla = theme::TEXT_PRIMARY.into();

        let display_text: SharedString = if text_empty {
            " ".into()
        } else {
            text_owned.clone().into()
        };

        let run_len = if text_empty { 1 } else { text_owned.len() };
        let runs = vec![TextRun {
            len: run_len,
            font: font.clone(),
            color: text_color,
            underline: None,
            strikethrough: None,
            background_color: None,
        }];

        let shaped: Option<ShapedLine> = Some(
            window
                .text_system()
                .shape_line(display_text, font_size, &runs, None),
        );

        let cursor_pos = shaped.as_ref().map(|line: &ShapedLine| {
            let x = if text_empty {
                px(0.0)
            } else {
                line.x_for_index(cursor_byte)
            };
            x - scroll_offset
        });

        let selection_rect = if has_sel {
            shaped.as_ref().map(|line: &ShapedLine| {
                let start_x = line.x_for_index(sel_range.start) - scroll_offset;
                let end_x = line.x_for_index(sel_range.end) - scroll_offset;
                Bounds::new(
                    point(bounds.origin.x + start_x, bounds.origin.y),
                    size(end_x - start_x, bounds.size.height),
                )
            })
        } else {
            None
        };

        TextInputPrepaintState {
            line: shaped,
            cursor_pos,
            selection_rect,
        }
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout_state: &mut (),
        prepaint_state: &mut TextInputPrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (is_focused, text_empty, scroll_offset, focus_handle, blink_visible) = {
            let state = self.state.read(cx);
            let focused = state.focus_handle.is_focused(window);
            let empty = state.text.is_empty();
            let offset = state.scroll_offset;
            let fh = state.focus_handle.clone();
            let blink_vis = state.blink_cursor.read(cx).visible();
            (focused, empty, offset, fh, blink_vis)
        };

        // Register as input handler for IME
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.state.clone()),
            cx,
        );

        // Clip to bounds
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            // Paint selection background
            if let Some(sel_rect) = &prepaint_state.selection_rect {
                let selection_color: Hsla = Hsla {
                    h: 0.72,
                    s: 0.6,
                    l: 0.4,
                    a: 0.4,
                };
                window.paint_quad(fill(*sel_rect, selection_color));
            }

            // Paint text
            if !text_empty {
                if let Some(line) = &prepaint_state.line {
                    let text_origin = point(bounds.origin.x - scroll_offset, bounds.origin.y);
                    line.paint(text_origin, bounds.size.height, window, cx).ok();
                }
            }

            // Paint cursor — only when focused AND blink says visible.
            // `blink_visible` is false when unfocused (BlinkCursor::stop sets it).
            if is_focused && blink_visible {
                if let Some(cursor_x) = prepaint_state.cursor_pos {
                    let cursor_color: Hsla = theme::TEXT_PRIMARY.into();
                    let cursor_h = px(15.0);
                    let cursor_y = bounds.origin.y + (bounds.size.height - cursor_h) / 2.0;
                    let cursor_bounds = Bounds::new(
                        point(bounds.origin.x + cursor_x, cursor_y),
                        size(CURSOR_WIDTH, cursor_h),
                    );
                    window.paint_quad(fill(cursor_bounds, cursor_color));
                }
            }
        });

        // Update scroll offset to keep cursor visible
        if is_focused {
            if let Some(cursor_x) = prepaint_state.cursor_pos {
                let new_offset = self.state.read(cx).scroll_offset;
                let content_width = bounds.size.width;
                let abs_cursor = cursor_x + new_offset;

                let mut updated_scroll = new_offset;
                if cursor_x < px(0.0) {
                    updated_scroll = abs_cursor;
                } else if cursor_x > content_width - CURSOR_WIDTH {
                    updated_scroll = abs_cursor - content_width + CURSOR_WIDTH;
                }
                if updated_scroll < px(0.0) {
                    updated_scroll = px(0.0);
                }
                if updated_scroll != new_offset {
                    self.state.update(cx, |state, _| {
                        state.scroll_offset = updated_scroll;
                    });
                    window.request_animation_frame();
                }
            }
        }

        // Store shaped line for IME bounds queries
        if let Some(line) = prepaint_state.line.take() {
            self.state.update(cx, |state, _| {
                state.last_shaped_line = Some(line);
                state.last_bounds = Some(bounds);
            });
        }
    }
}

// ── TextInput (the high-level wrapper component) ──────────────────────

/// A styled single-line text input widget.
///
/// Usage:
/// ```ignore
/// let input_state = cx.new(|cx| TextInputState::new(cx));
/// // In render:
/// TextInput::new(&input_state).placeholder("Search...")
/// ```
#[derive(IntoElement)]
pub struct TextInput {
    state: Entity<TextInputState>,
    placeholder: SharedString,
    prefix: Option<AnyElement>,
    suffix: Option<AnyElement>,
}

impl TextInput {
    pub fn new(state: &Entity<TextInputState>) -> Self {
        Self {
            state: state.clone(),
            placeholder: "".into(),
            prefix: None,
            suffix: None,
        }
    }

    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn prefix(mut self, el: impl IntoElement) -> Self {
        self.prefix = Some(el.into_any_element());
        self
    }

    pub fn suffix(mut self, el: impl IntoElement) -> Self {
        self.suffix = Some(el.into_any_element());
        self
    }
}

impl RenderOnce for TextInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focused = self.state.read(cx).focus_handle.is_focused(window);
        let text_empty = self.state.read(cx).text.is_empty();

        // Detect focus transitions → start / stop blink timer.
        self.state.update(cx, |state, cx| {
            state.update_focus_blink(focused, cx);
        });

        let focus_handle = self.state.read(cx).focus_handle.clone();
        let show_placeholder = text_empty && !self.placeholder.is_empty();

        div()
            .id(("text-input", self.state.entity_id()))
            .key_context("TextInput")
            .track_focus(&focus_handle)
            .w_full()
            .h(px(28.0))
            .px(px(10.0))
            .flex()
            .items_center()
            .gap(px(6.0))
            .bg(if focused {
                theme::BG_ELEVATED
            } else {
                theme::BG_INPUT
            })
            .border_1()
            .border_color(if focused {
                theme::TWITCH_PURPLE
            } else {
                theme::BORDER_SUBTLE
            })
            .rounded(px(6.0))
            .cursor_text()
            // Actions
            .on_action(window.listener_for(&self.state, TextInputState::action_backspace))
            .on_action(window.listener_for(&self.state, TextInputState::action_delete))
            .on_action(window.listener_for(&self.state, TextInputState::action_delete_prev_word))
            .on_action(window.listener_for(&self.state, TextInputState::action_delete_to_end))
            .on_action(window.listener_for(&self.state, TextInputState::action_move_left))
            .on_action(window.listener_for(&self.state, TextInputState::action_move_right))
            .on_action(window.listener_for(&self.state, TextInputState::action_move_home))
            .on_action(window.listener_for(&self.state, TextInputState::action_move_end))
            .on_action(window.listener_for(&self.state, TextInputState::action_move_prev_word))
            .on_action(window.listener_for(&self.state, TextInputState::action_move_next_word))
            .on_action(window.listener_for(&self.state, TextInputState::action_select_left))
            .on_action(window.listener_for(&self.state, TextInputState::action_select_right))
            .on_action(window.listener_for(&self.state, TextInputState::action_select_all))
            .on_action(window.listener_for(&self.state, TextInputState::action_select_to_start))
            .on_action(window.listener_for(&self.state, TextInputState::action_select_to_end))
            .on_action(window.listener_for(&self.state, TextInputState::action_select_prev_word))
            .on_action(window.listener_for(&self.state, TextInputState::action_select_next_word))
            .on_action(window.listener_for(&self.state, TextInputState::action_copy))
            .on_action(window.listener_for(&self.state, TextInputState::action_cut))
            .on_action(window.listener_for(&self.state, TextInputState::action_paste))
            .on_action(window.listener_for(&self.state, TextInputState::action_enter))
            .on_action(window.listener_for(&self.state, TextInputState::action_escape))
            .on_click({
                let state = self.state.clone();
                move |_event, window, cx| {
                    let handle = state.read(cx).focus_handle.clone();
                    window.focus(&handle);
                }
            })
            // Prefix
            .children(self.prefix)
            // Input element + placeholder
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .relative()
                    .h_full()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .w_full()
                            .h_full()
                            .relative()
                            .flex()
                            .items_center()
                            // Always render the TextInputElement (it paints cursor + text).
                            .child(self.state.clone())
                            // Placeholder overlays the input area. Shown when empty,
                            // whether focused or not (like a real text field).
                            .when(show_placeholder, |el: Div| {
                                el.child(
                                    gpui::div()
                                        .absolute()
                                        .top_0()
                                        .left_0()
                                        .h_full()
                                        .flex()
                                        .items_center()
                                        .text_color(theme::TEXT_DISABLED)
                                        .text_size(px(13.0))
                                        .child(self.placeholder.clone()),
                                )
                            }),
                    ),
            )
            // Suffix
            .children(self.suffix)
    }
}
