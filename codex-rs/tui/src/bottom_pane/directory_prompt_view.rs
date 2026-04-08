use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use codex_utils_absolute_path::AbsolutePathBuf;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::Widget;

use crate::key_hint;
use crate::render::renderable::Renderable;
use crate::tui::FrameRequester;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::standard_popup_hint_line;
use super::textarea::TextArea;
use super::textarea::TextAreaState;

const DIRECTORY_SUGGESTION_LIMIT: usize = 6;
const DIRECTORY_SUGGESTION_DEBOUNCE: Duration = Duration::from_millis(120);

pub(crate) type DirectoryPromptSubmitted = Box<dyn Fn(String) + Send + Sync>;
pub(crate) type DirectoryPromptCancelled = Box<dyn Fn() + Send + Sync>;

pub(crate) struct DirectoryPromptView {
    title: String,
    placeholder: String,
    context_label: Option<String>,
    cwd: PathBuf,
    frame_requester: FrameRequester,
    on_submit: DirectoryPromptSubmitted,
    on_cancel: DirectoryPromptCancelled,
    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    suggestions: RefCell<DirectorySuggestionState>,
    complete: bool,
}

struct DirectorySuggestionState {
    items: Vec<PathBuf>,
    selected: Option<usize>,
    pending_refresh_at: Option<Instant>,
}

impl DirectorySuggestionState {
    fn new(items: Vec<PathBuf>) -> Self {
        let selected = (!items.is_empty()).then_some(0);
        Self {
            items,
            selected,
            pending_refresh_at: None,
        }
    }
}

impl DirectoryPromptView {
    pub(crate) fn new(
        title: String,
        placeholder: String,
        context_label: Option<String>,
        cwd: PathBuf,
        frame_requester: FrameRequester,
        on_submit: DirectoryPromptSubmitted,
        on_cancel: DirectoryPromptCancelled,
    ) -> Self {
        let initial_suggestions = list_directory_suggestions("", &cwd);
        Self {
            title,
            placeholder,
            context_label,
            cwd,
            frame_requester,
            on_submit,
            on_cancel,
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            suggestions: RefCell::new(DirectorySuggestionState::new(initial_suggestions)),
            complete: false,
        }
    }

    fn queue_suggestion_refresh(&self) {
        let refresh_at = Instant::now() + DIRECTORY_SUGGESTION_DEBOUNCE;
        self.suggestions.borrow_mut().pending_refresh_at = Some(refresh_at);
        self.frame_requester
            .schedule_frame_in(DIRECTORY_SUGGESTION_DEBOUNCE);
    }

    fn refresh_suggestions_if_due(&self) {
        let pending_refresh_at = self.suggestions.borrow().pending_refresh_at;
        let Some(refresh_at) = pending_refresh_at else {
            return;
        };

        let now = Instant::now();
        if now < refresh_at {
            self.frame_requester
                .schedule_frame_in(refresh_at.saturating_duration_since(now));
            return;
        }

        let suggestions = list_directory_suggestions(self.textarea.text(), &self.cwd);
        let selected = (!suggestions.is_empty()).then_some(0);
        let mut state = self.suggestions.borrow_mut();
        state.items = suggestions;
        state.selected = selected;
        state.pending_refresh_at = None;
    }

    fn move_selection(&self, step: isize) -> bool {
        self.refresh_suggestions_if_due();
        let mut state = self.suggestions.borrow_mut();
        if state.items.is_empty() {
            return false;
        }
        let len = state.items.len() as isize;
        let current = state.selected.unwrap_or(0) as isize;
        let next = (current + step).rem_euclid(len) as usize;
        state.selected = Some(next);
        true
    }

    fn apply_selected_suggestion(&mut self) -> bool {
        self.refresh_suggestions_if_due();
        let selected = {
            let state = self.suggestions.borrow();
            state
                .selected
                .and_then(|index| state.items.get(index))
                .cloned()
        };
        let Some(selected) = selected else {
            return false;
        };

        let replacement = selected.display().to_string();
        self.textarea.set_text_clearing_elements(&replacement);
        self.textarea.set_cursor(replacement.len());
        let refreshed = list_directory_suggestions(&replacement, &self.cwd);
        let mut state = self.suggestions.borrow_mut();
        state.items = refreshed;
        state.selected = (!state.items.is_empty()).then_some(0);
        state.pending_refresh_at = None;
        true
    }

    fn suggestion_lines(&self) -> Vec<Line<'static>> {
        self.refresh_suggestions_if_due();
        let state = self.suggestions.borrow();
        state
            .items
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let selected = state.selected == Some(index);
                let marker = if selected { "› ".cyan() } else { "  ".dim() };
                let path_text = path.display().to_string();
                let path_span = if selected {
                    path_text.cyan().bold()
                } else {
                    path_text.dim()
                };
                Line::from(vec![gutter(), marker, path_span])
            })
            .collect()
    }

    fn hint_lines(&self) -> [Line<'static>; 2] {
        [
            Line::from(vec![
                "Use ".into(),
                key_hint::plain(KeyCode::Tab).into(),
                " to complete and ".into(),
                key_hint::plain(KeyCode::Up).into(),
                "/".into(),
                key_hint::plain(KeyCode::Down).into(),
                " to choose a directory".into(),
            ]),
            standard_popup_hint_line(),
        ]
    }

    fn input_height(&self, width: u16) -> u16 {
        let usable_width = width.saturating_sub(2);
        let text_height = self.textarea.desired_height(usable_width).clamp(1, 4);
        text_height.saturating_add(1).min(5)
    }
}

impl BottomPaneView for DirectoryPromptView {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let text = self.textarea.text().trim().to_string();
                if !text.is_empty() {
                    (self.on_submit)(text);
                    self.complete = true;
                }
            }
            KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                self.apply_selected_suggestion();
            }
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if !self.move_selection(-1) {
                    self.textarea.input(key_event);
                }
            }
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if !self.move_selection(1) {
                    self.textarea.input(key_event);
                }
            }
            other => {
                let before = self.textarea.text().to_string();
                self.textarea.input(other);
                if self.textarea.text() != before {
                    self.queue_suggestion_refresh();
                }
            }
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        (self.on_cancel)();
        self.complete = true;
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn handle_paste(&mut self, pasted: String) -> bool {
        if pasted.is_empty() {
            return false;
        }
        self.textarea.insert_str(&pasted);
        self.queue_suggestion_refresh();
        true
    }
}

impl Renderable for DirectoryPromptView {
    fn desired_height(&self, width: u16) -> u16 {
        let extra_top: u16 = if self.context_label.is_some() { 1 } else { 0 };
        let suggestions_height = self.suggestion_lines().len() as u16;
        1u16 + extra_top + self.input_height(width) + suggestions_height + 3u16
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let input_height = self.input_height(area.width);
        let suggestion_lines = self.suggestion_lines();

        let title_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        Paragraph::new(Line::from(vec![gutter(), self.title.clone().bold()]))
            .render(title_area, buf);

        let mut input_y = area.y.saturating_add(1);
        if let Some(context_label) = &self.context_label {
            let context_area = Rect {
                x: area.x,
                y: input_y,
                width: area.width,
                height: 1,
            };
            Paragraph::new(Line::from(vec![gutter(), context_label.clone().cyan()]))
                .render(context_area, buf);
            input_y = input_y.saturating_add(1);
        }

        let input_area = Rect {
            x: area.x,
            y: input_y,
            width: area.width,
            height: input_height,
        };
        if input_area.width >= 2 {
            for row in 0..input_area.height {
                Paragraph::new(Line::from(vec![gutter()])).render(
                    Rect {
                        x: input_area.x,
                        y: input_area.y.saturating_add(row),
                        width: 2,
                        height: 1,
                    },
                    buf,
                );
            }

            let text_area_height = input_area.height.saturating_sub(1);
            if text_area_height > 0 {
                if input_area.width > 2 {
                    Clear.render(
                        Rect {
                            x: input_area.x.saturating_add(2),
                            y: input_area.y,
                            width: input_area.width.saturating_sub(2),
                            height: 1,
                        },
                        buf,
                    );
                }
                let textarea_rect = Rect {
                    x: input_area.x.saturating_add(2),
                    y: input_area.y.saturating_add(1),
                    width: input_area.width.saturating_sub(2),
                    height: text_area_height,
                };
                let mut state = self.textarea_state.borrow_mut();
                StatefulWidgetRef::render_ref(&(&self.textarea), textarea_rect, buf, &mut state);
                if self.textarea.text().is_empty() {
                    Paragraph::new(Line::from(self.placeholder.clone().dim()))
                        .render(textarea_rect, buf);
                }
            }
        }

        let mut row_y = input_area.y.saturating_add(input_height);
        for line in &suggestion_lines {
            if row_y >= area.y.saturating_add(area.height) {
                break;
            }
            Paragraph::new(line.clone()).render(
                Rect {
                    x: area.x,
                    y: row_y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
            row_y = row_y.saturating_add(1);
        }

        let hint_lines = self.hint_lines();
        for hint in &hint_lines {
            if row_y >= area.y.saturating_add(area.height) {
                break;
            }
            Paragraph::new(hint.clone()).render(
                Rect {
                    x: area.x,
                    y: row_y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
            row_y = row_y.saturating_add(1);
        }
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        if area.height < 2 || area.width <= 2 {
            return None;
        }
        let text_area_height = self.input_height(area.width).saturating_sub(1);
        if text_area_height == 0 {
            return None;
        }
        let extra_offset: u16 = if self.context_label.is_some() { 1 } else { 0 };
        let top_line_count = 1u16 + extra_offset;
        let textarea_rect = Rect {
            x: area.x.saturating_add(2),
            y: area.y.saturating_add(top_line_count).saturating_add(1),
            width: area.width.saturating_sub(2),
            height: text_area_height,
        };
        let state = *self.textarea_state.borrow();
        self.textarea.cursor_pos_with_state(textarea_rect, state)
    }
}

fn list_directory_suggestions(input: &str, cwd: &Path) -> Vec<PathBuf> {
    let trimmed = input.trim();
    let (search_root, prefix) = resolve_suggestion_root(trimmed, cwd);
    let Some(search_root) = search_root else {
        return Vec::new();
    };

    let prefix_lower = prefix.to_ascii_lowercase();
    let mut suggestions = fs::read_dir(search_root.as_path())
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            entry
                .file_type()
                .ok()
                .filter(|file_type| file_type.is_dir())
                .map(|_| path)
        })
        .filter(|path| {
            if prefix_lower.is_empty() {
                return true;
            }
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.to_ascii_lowercase().starts_with(&prefix_lower))
        })
        .collect::<Vec<_>>();

    suggestions.sort_by_key(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_ascii_lowercase())
            .unwrap_or_else(|| path.display().to_string())
    });
    suggestions.truncate(DIRECTORY_SUGGESTION_LIMIT);
    suggestions
}

fn resolve_suggestion_root(input: &str, cwd: &Path) -> (Option<AbsolutePathBuf>, String) {
    if input.is_empty() {
        return (
            Some(AbsolutePathBuf::resolve_path_against_base(".", cwd)),
            String::new(),
        );
    }

    let normalized = AbsolutePathBuf::resolve_path_against_base(input, cwd);

    if input.ends_with(std::path::MAIN_SEPARATOR)
        || input.ends_with('/')
        || input.ends_with('\\')
        || matches!(input, "~" | "." | "..")
    {
        return (Some(normalized), String::new());
    }

    let prefix = normalized
        .as_path()
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_default();
    (normalized.parent(), prefix)
}

fn gutter() -> Span<'static> {
    "▌ ".cyan()
}
