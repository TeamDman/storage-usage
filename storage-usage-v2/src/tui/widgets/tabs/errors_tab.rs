use crate::tui::progress::MftFileProgress;
use crate::tui::widgets::tabs::keyboard_response::KeyboardResponse;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::collections::HashMap;

pub struct ErrorsTab {
    scroll_offset: usize,
    selected_index: usize,
    show_grouped: bool,
    cached_grouped: Vec<(String, usize, Vec<usize>)>, // (message, count, indices)
}

impl Default for ErrorsTab { fn default() -> Self { Self::new() } }

impl ErrorsTab {
    pub fn new() -> Self {
        Self { scroll_offset: 0, selected_index: 0, show_grouped: true, cached_grouped: Vec::new() }
    }

    pub fn on_key(&mut self, event: KeyEvent) -> KeyboardResponse {
        match event.code {
            KeyCode::Char('g') => { self.show_grouped = !self.show_grouped; KeyboardResponse::Consume }
            KeyCode::Up => { if self.selected_index>0 { self.selected_index -=1; if self.selected_index < self.scroll_offset { self.scroll_offset = self.selected_index; }} KeyboardResponse::Consume }
            KeyCode::Down => { self.selected_index = self.selected_index.saturating_add(1); KeyboardResponse::Consume }
            KeyCode::PageUp => { self.selected_index = self.selected_index.saturating_sub(10); self.scroll_offset = self.scroll_offset.saturating_sub(10); KeyboardResponse::Consume }
            KeyCode::PageDown => { self.selected_index = self.selected_index.saturating_add(10); KeyboardResponse::Consume }
            KeyCode::Home => { self.selected_index = 0; self.scroll_offset = 0; KeyboardResponse::Consume }
            KeyCode::End => { self.selected_index = usize::MAX; KeyboardResponse::Consume }
            _ => KeyboardResponse::Pass,
        }
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, mft_files: &[MftFileProgress]) {
        // Collect errors from all files
        let mut all_errors: Vec<(usize, &Line<'static>)> = Vec::new();
        for (file_idx, file) in mft_files.iter().enumerate() {
            for error in &file.errors {
                all_errors.push((file_idx, error));
            }
        }

        // Rebuild grouped cache if lengths changed
        if self.cached_grouped.iter().map(|(_,c,_)| *c).sum::<usize>() != all_errors.len() {
            let mut map: HashMap<String, (usize, Vec<usize>)> = HashMap::new();
            for (_i,(file_idx, line)) in all_errors.iter().enumerate() {
                let mut msg = String::new();
                for span in &line.spans { msg.push_str(&span.content); }
                // include file index tag to differentiate same error across files? choose not to for grouping identical text
                let entry = map.entry(msg).or_insert((0, Vec::new()));
                entry.0 +=1; entry.1.push(*file_idx);
            }
            self.cached_grouped = map.into_iter().map(|(msg,(count, indices))| (msg, count, indices)).collect();
            self.cached_grouped.sort_by(|a,b| b.1.cmp(&a.1));
        }

        let header = if self.show_grouped { "Errors (grouped, press 'g' to toggle)" } else { "Errors (raw, press 'g' to toggle)" };
        Paragraph::new(header).render(Rect { x: area.x, y: area.y, width: area.width, height: 1 }, buf);

        let list_area = Rect { x: area.x, y: area.y+1, width: area.width, height: area.height.saturating_sub(1) };

        if self.show_grouped { self.render_grouped(list_area, buf, &mft_files); } else { self.render_raw(list_area, buf, &mft_files); }
    }

    fn render_grouped(&mut self, area: Rect, buf: &mut Buffer, _mft_files: &[MftFileProgress]) {
        if self.cached_grouped.is_empty() {
            Paragraph::new("No errors recorded").style(Style::default().fg(Color::Green)).render(area, buf); return;
        }
        let visible_height = area.height as usize;
        if visible_height==0 { return; }
        let len = self.cached_grouped.len();
        self.selected_index = self.selected_index.min(len.saturating_sub(1));
        let max_scroll = len.saturating_sub(visible_height);
        self.scroll_offset = self.scroll_offset.min(max_scroll);
        if self.selected_index >= self.scroll_offset + visible_height { self.scroll_offset = self.selected_index - visible_height +1; }

        let items: Vec<ListItem> = self.cached_grouped.iter().enumerate().skip(self.scroll_offset).take(visible_height).map(|(idx,(msg,count, indices))| {
            let style = if idx==self.selected_index { Style::default().fg(Color::Black).bg(Color::Yellow) } else { Style::default() };
            let file_count = indices.len();
            let display = format!("[{count}x across {file_count} file(s)] {msg}");
            ListItem::new(Line::from(Span::styled(display, style)))
        }).collect();
        List::new(items).render(area, buf);
    }

    fn render_raw(&mut self, area: Rect, buf: &mut Buffer, mft_files: &[MftFileProgress]) {
        let mut raw: Vec<(usize,String)> = Vec::new();
        for (file_idx, file) in mft_files.iter().enumerate() { for line in &file.errors { let mut msg=String::new(); for span in &line.spans { msg.push_str(&span.content); } raw.push((file_idx, msg)); }}
        if raw.is_empty() { Paragraph::new("No errors recorded").style(Style::default().fg(Color::Green)).render(area, buf); return; }
        let visible_height = area.height as usize; if visible_height==0 { return; }
        let len = raw.len(); self.selected_index = self.selected_index.min(len.saturating_sub(1));
        let max_scroll = len.saturating_sub(visible_height); self.scroll_offset = self.scroll_offset.min(max_scroll);
        if self.selected_index >= self.scroll_offset + visible_height { self.scroll_offset = self.selected_index - visible_height +1; }
        let items: Vec<ListItem> = raw.iter().enumerate().skip(self.scroll_offset).take(visible_height).map(|(idx,(file_idx,msg))| {
            let style = if idx==self.selected_index { Style::default().fg(Color::Black).bg(Color::Yellow) } else { Style::default() };
            let file_name = mft_files[*file_idx].path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            let display = format!("[{file_name}] {msg}");
            ListItem::new(Line::from(Span::styled(display, style)))
        }).collect();
        List::new(items).render(area, buf);
    }
}
