use crate::{CookieApp, resources::TextCatalog, theme::number};
use debugger::{
    ScanFilter, Scanner, ValueType, Watch, diff, parse_hex, parse_u64, pointer_label, unsigned,
};
use eframe::egui::{self, Button, RichText, ScrollArea};
use egui_dock::{DockArea, DockState, TabViewer};
use vm_core::{Address, Architecture, ExecutionBackend, Memory, RegionKind};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum Tab {
    Memory,
    Scanner,
    Inspector,
    Watch,
    History,
    Diff,
}
impl Tab {
    fn text_key(self) -> &'static str {
        match self {
            Self::Memory => "tool.memory",
            Self::Scanner => "tool.scanner",
            Self::Inspector => "tool.inspector",
            Self::Watch => "tool.watch",
            Self::History => "tool.history",
            Self::Diff => "tool.diff",
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScanFormat {
    Number,
    Text,
    HexBytes,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemoryFormat {
    Bytes,
    Qwords,
}
impl MemoryFormat {
    fn row_bytes(self) -> u64 {
        match self {
            Self::Bytes => 16,
            Self::Qwords => 32,
        }
    }
}
impl ScanFormat {
    fn text_key(self) -> &'static str {
        match self {
            Self::Number => "scanner.input.number",
            Self::Text => "scanner.input.text",
            Self::HexBytes => "scanner.input.hex",
        }
    }
    fn bytes(self, value: &str) -> Result<Vec<u8>, String> {
        match self {
            Self::Text => Ok(value.as_bytes().to_vec()),
            Self::HexBytes => parse_hex(value),
            Self::Number => Err("select text or bytes".into()),
        }
    }
    fn display(self, bytes: &[u8]) -> String {
        match self {
            Self::Number => number(unsigned(bytes)),
            Self::Text => String::from_utf8_lossy(bytes).escape_debug().to_string(),
            Self::HexBytes => hex(bytes),
        }
    }
}
pub(crate) struct DebugWorkspace {
    dock: DockState<Tab>,
    pending_tools: Vec<Tab>,
    selected: Address,
    anchor: Address,
    selection_end: Address,
    jump: String,
    scroll_to: Option<f32>,
    back: Vec<Address>,
    bookmarks: Vec<Address>,
    symbols: bool,
    scanner: Scanner,
    scan_value: String,
    scan_width: usize,
    scan_unknown: bool,
    scan_aligned: bool,
    scan_filter: ScanFilter,
    watches: Vec<Watch>,
    watch_name: String,
    snapshot: Option<Memory>,
    diffs: Vec<debugger::Diff>,
    inline_edit: Option<(Address, String)>,
    memory_format: MemoryFormat,
    memory_region: RegionKind,
    scan_format: ScanFormat,
    result_format: ScanFormat,
    result_page: usize,
}
fn layout() -> DockState<Tab> {
    let mut dock = DockState::new(vec![]);
    *dock.main_surface_mut() = egui_dock::Tree::default();
    dock
}
impl DebugWorkspace {
    pub(crate) fn new() -> Self {
        Self {
            dock: layout(),
            pending_tools: vec![],
            selected: Architecture::HEAP,
            anchor: Architecture::HEAP,
            selection_end: Architecture::HEAP,
            jump: format!("0x{:016X}", Architecture::HEAP),
            scroll_to: None,
            back: vec![],
            bookmarks: vec![],
            symbols: false,
            scanner: Scanner::default(),
            scan_value: "0".into(),
            scan_width: 8,
            scan_unknown: false,
            scan_aligned: true,
            scan_filter: ScanFilter::Exact,
            watches: vec![],
            watch_name: String::new(),
            snapshot: None,
            diffs: vec![],
            inline_edit: None,
            memory_format: MemoryFormat::Bytes,
            memory_region: RegionKind::Heap,
            scan_format: ScanFormat::Number,
            result_format: ScanFormat::Number,
            result_page: 0,
        }
    }
    pub(crate) fn menu(&mut self, ui: &mut egui::Ui, text: &TextCatalog) {
        ui.menu_button(text.get("tools.menu"), |ui| {
            if ui.button(text.get("tools.find")).clicked() {
                self.open_tool(Tab::Scanner, ui.ctx());
                ui.close();
            }
            if ui.button(text.get("tools.memory")).clicked() {
                self.open_tool(Tab::Memory, ui.ctx());
                ui.close();
            }
            ui.menu_button(text.get("tools.more"), |ui| {
                for tab in [Tab::Watch, Tab::History, Tab::Diff] {
                    if ui.button(text.get(tab.text_key())).clicked() {
                        self.open_tool(tab, ui.ctx());
                        ui.close();
                    }
                }
            });
            if ui.button(text.get("tools.close_all")).clicked() {
                self.dock = layout();
                ui.close();
            }
        });
    }
    fn open_tool(&mut self, tab: Tab, ctx: &egui::Context) {
        if let Some(path) = self.dock.find_tab(&tab) {
            let _ = self.dock.set_active_tab(path);
            return;
        }
        let bounds = ctx.content_rect();
        let count = self.dock.iter_all_tabs().count();
        let width: f32 = if tab == Tab::Memory { 760.0 } else { 420.0 };
        let size = egui::vec2(
            width.min(bounds.width() - 32.0),
            (bounds.height() - 120.0).clamp(240.0, 560.0),
        );
        let surface = self.dock.add_window(vec![tab]);
        if let Some(window) = self.dock.get_window_state_mut(surface) {
            window.set_position(egui::pos2(
                (bounds.right() - size.x - 24.0).max(16.0),
                64.0 + (count % 3) as f32 * 24.0,
            ));
            window.set_size(size);
        }
    }
    pub(crate) fn show(&mut self, ui: &mut egui::Ui, app: &mut CookieApp) {
        let mut dock = std::mem::replace(&mut self.dock, layout());
        // Only floating surfaces are populated; the game keeps the entire main surface.
        let mut host = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
            ui.max_rect().min,
            egui::Vec2::ZERO,
        )));
        DockArea::new(&mut dock)
            .style(egui_dock::Style::from_egui(ui.style()))
            .show_inside(
                &mut host,
                &mut Viewer {
                    workspace: self,
                    app,
                },
            );
        self.dock = dock;
        for tab in std::mem::take(&mut self.pending_tools) {
            self.open_tool(tab, ui.ctx());
        }
    }
    fn navigate(&mut self, a: Address, app: &mut CookieApp) {
        if let Some(r) = app
            .machine
            .memory_regions()
            .iter()
            .find(|r| matches!(r.kind(), RegionKind::Heap | RegionKind::Code) && r.contains(a))
        {
            if a != self.selected {
                self.back.push(self.selected);
                if self.back.len() > 128 {
                    self.back.remove(0);
                }
            }
            self.pending_tools.push(Tab::Memory);
            self.memory_region = r.kind();
            self.selected = a;
            self.anchor = a;
            self.selection_end = a;
            self.jump = format!("0x{a:016X}");
            self.inline_edit = None;
            self.scroll_to = Some(((a - r.base()) / self.memory_format.row_bytes()) as f32 * 32.0);
        } else {
            app.report::<(), _>(Err(
                app.format_text("memory.outside", &[("address", format!("{a:016X}"))])
            ));
        }
    }
    fn selection(&self) -> (Address, u64) {
        let a = self.anchor.min(self.selection_end);
        let end = self.anchor.max(self.selection_end);
        (a, (end - a + 1).min(4096))
    }
    fn follow(&mut self, app: &mut CookieApp) {
        match app.machine.read_memory(self.selected, 8) {
            Ok(b) => self.navigate(unsigned(&b), app),
            Err(e) => app.report::<(), _>(Err(e)),
        }
    }
    fn add_watch(&mut self, kind: ValueType) {
        if !self
            .watches
            .iter()
            .any(|w| w.address == self.selected && w.kind == kind)
        {
            self.watches.push(Watch {
                address: self.selected,
                kind,
                name: if self.watch_name.is_empty() {
                    kind.label().into()
                } else {
                    self.watch_name.clone()
                },
            });
        }
    }
    fn memory_ui(&mut self, ui: &mut egui::Ui, app: &mut CookieApp) {
        let text = app.resources.text.clone();
        ui.horizontal_wrapped(|ui| {
            let previous_region = self.memory_region;
            ui.selectable_value(
                &mut self.memory_region,
                RegionKind::Heap,
                text.get("memory.region.heap"),
            );
            ui.selectable_value(
                &mut self.memory_region,
                RegionKind::Code,
                text.get("memory.region.code"),
            );
            if previous_region != self.memory_region {
                let base = match self.memory_region {
                    RegionKind::Code => Architecture::CODE,
                    _ => Architecture::HEAP,
                };
                self.selected = base;
                self.anchor = base;
                self.selection_end = base;
                self.jump = format!("0x{base:016X}");
                self.inline_edit = None;
                self.scroll_to = Some(0.0);
            }
            ui.separator();
            ui.label(text.get("memory.address"));
            ui.add(
                egui::TextEdit::singleline(&mut self.jump)
                    .desired_width(190.0)
                    .font(egui::TextStyle::Monospace),
            );
            if ui.button(text.get("memory.go")).clicked() {
                match parse_u64(&self.jump) {
                    Ok(a) => self.navigate(a, app),
                    Err(e) => app.report::<(), _>(Err(e)),
                }
            }
            if ui
                .add_enabled(!self.back.is_empty(), Button::new(text.get("memory.back")))
                .clicked()
                && let Some(a) = self.back.pop()
            {
                let history = self.back.clone();
                self.navigate(a, app);
                self.back = history;
            }
            if ui.button(text.get("memory.follow")).clicked() {
                self.follow(app);
            }
            if ui.button(text.get("memory.inspector")).clicked() {
                self.pending_tools.push(Tab::Inspector);
            }
            ui.separator();
            let old_format = self.memory_format;
            ui.selectable_value(
                &mut self.memory_format,
                MemoryFormat::Bytes,
                text.get("memory.format.bytes"),
            );
            ui.selectable_value(
                &mut self.memory_format,
                MemoryFormat::Qwords,
                text.get("memory.format.qwords"),
            )
            .on_hover_text(text.get("memory.format.qwords_hint"));
            if old_format != self.memory_format {
                self.inline_edit = None;
                let region_base = match self.memory_region {
                    RegionKind::Code => Architecture::CODE,
                    _ => Architecture::HEAP,
                };
                self.scroll_to = Some(
                    ((self.selected - region_base) / self.memory_format.row_bytes()) as f32 * 32.0,
                );
            }
        });
        let (start, n) = self.selection();
        let Some(r) = app
            .machine
            .memory_regions()
            .iter()
            .find(|r| r.kind() == self.memory_region)
        else {
            ui.label(
                app.format_text(
                    "memory.no_region",
                    &[(
                        "region",
                        text.get(match self.memory_region {
                            RegionKind::Code => "memory.region.code",
                            _ => "memory.region.heap",
                        })
                        .to_owned(),
                    )],
                ),
            );
            return;
        };
        let base = r.base();
        let len = r.len();
        let writable = r.permissions().write;
        let row_bytes = self.memory_format.row_bytes();
        let rows = len.div_ceil(row_bytes) as usize;
        let changed = self
            .snapshot
            .as_ref()
            .map(|s| {
                diff(s, &app.machine)
                    .into_iter()
                    .map(|d| d.address)
                    .collect::<std::collections::HashSet<_>>()
            })
            .unwrap_or_default();
        let mut scroll = ScrollArea::both()
            .id_salt("hex_scroll")
            .auto_shrink([false, false]);
        if let Some(offset) = self.scroll_to.take() {
            scroll = scroll.vertical_scroll_offset(offset);
        }
        let mut edit_commit = None;
        let mut cancel_edit = false;
        scroll.show_rows(ui, 24.0, rows, |ui, range| {
            ui.set_min_width(740.0);
            ui.spacing_mut().button_padding = egui::Vec2::ZERO;
            ui.spacing_mut().item_spacing.x = 4.0;
            for row in range {
                let a = base + row as u64 * row_bytes;
                let count = (len - row as u64 * row_bytes).min(row_bytes);
                let bytes = match app.machine.read_memory(a, count) {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{a:016X}"))
                            .monospace()
                            .color(app.theme().muted),
                    );
                    if self.memory_format == MemoryFormat::Qwords {
                        for (i, chunk) in bytes.as_chunks::<8>().0.iter().enumerate() {
                            let addr = a + i as u64 * 8;
                            let value = u64::from_le_bytes(*chunk);
                            let selected = addr < start + n && addr + 8 > start;
                            let changed_qword =
                                (0..8).any(|offset| changed.contains(&(addr + offset)));
                            if let Some((editing, text)) = &mut self.inline_edit
                                && *editing == addr
                            {
                                let id = egui::Id::new(("qword_edit", addr));
                                let response = ui.add(
                                    egui::TextEdit::singleline(text)
                                        .id(id)
                                        .font(egui::TextStyle::Monospace)
                                        .desired_width(142.0)
                                        .hint_text(format!("{value:016X}"))
                                        .char_limit(18),
                                );
                                if response.has_focus()
                                    && ui.input(|input| input.key_pressed(egui::Key::Escape))
                                {
                                    cancel_edit = true;
                                } else if response.changed()
                                    && response.has_focus()
                                    && qword_bytes(text, true).is_ok()
                                {
                                    edit_commit =
                                        Some((addr, text.clone(), true, MemoryFormat::Qwords));
                                } else if (response.has_focus()
                                    && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                                    || response.lost_focus()
                                {
                                    edit_commit =
                                        Some((addr, text.clone(), false, MemoryFormat::Qwords));
                                }
                                continue;
                            }
                            let mut cell_text = RichText::new(format!("{value:016X}")).monospace();
                            if changed_qword {
                                cell_text = cell_text.background_color(app.theme().changed);
                            }
                            let response = ui.add(
                                Button::new(cell_text)
                                    .selected(selected)
                                    .frame(selected)
                                    .min_size(egui::vec2(142.0, 24.0)),
                            );
                            let response = if writable {
                                response
                            } else {
                                response.on_hover_text(text.get("memory.read_only"))
                            };
                            if response.clicked() {
                                self.selected = addr;
                                if !ui.input(|input| input.modifiers.shift) {
                                    self.anchor = addr;
                                }
                                self.selection_end = addr + 7;
                                self.jump = format!("0x{addr:016X}");
                            }
                            if writable && response.double_clicked() {
                                self.inline_edit = Some((addr, String::new()));
                                ui.memory_mut(|memory| {
                                    memory.request_focus(egui::Id::new(("qword_edit", addr)))
                                });
                            }
                            response.context_menu(|ui| {
                                if ui.button(text.get("memory.copy_address")).clicked() {
                                    ui.ctx().copy_text(format!("0x{addr:016X}"));
                                    ui.close();
                                }
                                if ui.button(text.get("memory.copy_qword")).clicked() {
                                    ui.ctx().copy_text(format!("0x{value:016X}"));
                                    ui.close();
                                }
                                if ui.button(text.get("memory.bookmark")).clicked() {
                                    if !self.bookmarks.contains(&addr) {
                                        self.bookmarks.push(addr);
                                    }
                                    ui.close();
                                }
                                if ui.button(text.get("memory.inspect_qword")).clicked() {
                                    self.selected = addr;
                                    self.pending_tools.push(Tab::Inspector);
                                    ui.close();
                                }
                                if ui.button(text.get("memory.follow_qword")).clicked() {
                                    self.selected = addr;
                                    self.follow(app);
                                    ui.close();
                                }
                                if ui.button(text.get("memory.watch_u64")).clicked() {
                                    self.selected = addr;
                                    self.add_watch(ValueType::U64);
                                    ui.close();
                                }
                            });
                        }
                    } else {
                        for (i, b) in bytes.iter().enumerate() {
                            let addr = a + i as u64;
                            let selected = addr >= start && addr < start + n;
                            if i == 8 {
                                ui.add_space(8.0);
                            }
                            let mut cell_text = RichText::new(format!("{b:02X}")).monospace();
                            if changed.contains(&addr) {
                                cell_text = cell_text.background_color(app.theme().changed);
                            }
                            if let Some((editing, text)) = &mut self.inline_edit
                                && *editing == addr
                            {
                                let id = egui::Id::new(("byte_edit", addr));
                                let response = ui.add(
                                    egui::TextEdit::singleline(text)
                                        .id(id)
                                        .font(egui::TextStyle::Monospace)
                                        .desired_width(24.0)
                                        .hint_text(format!("{b:02X}"))
                                        .char_limit(512),
                                );
                                if response.has_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Escape))
                                {
                                    cancel_edit = true;
                                } else if response.changed()
                                    && response.has_focus()
                                    && parse_hex(text).is_ok_and(|b| !b.is_empty())
                                {
                                    edit_commit =
                                        Some((addr, text.clone(), true, MemoryFormat::Bytes));
                                } else if (response.has_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                    || response.lost_focus()
                                {
                                    edit_commit =
                                        Some((addr, text.clone(), false, MemoryFormat::Bytes));
                                }
                                continue;
                            }
                            let response = ui.add(
                                Button::new(cell_text)
                                    .selected(selected)
                                    .frame(selected)
                                    .min_size(egui::vec2(24.0, 24.0)),
                            );
                            let response = if writable {
                                response
                            } else {
                                response.on_hover_text(text.get("memory.read_only"))
                            };
                            if response.clicked() {
                                if ui.input(|i| i.modifiers.shift) {
                                    self.selection_end = addr;
                                } else {
                                    self.anchor = addr;
                                    self.selection_end = addr;
                                }
                                self.selected = addr;
                                self.jump = format!("0x{addr:016X}");
                            }
                            if writable && response.double_clicked() {
                                self.inline_edit = Some((addr, String::new()));
                                ui.memory_mut(|m| {
                                    m.request_focus(egui::Id::new(("byte_edit", addr)))
                                });
                            }
                            response.context_menu(|ui| {
                                if ui.button(text.get("memory.copy_address")).clicked() {
                                    ui.ctx().copy_text(format!("0x{addr:016X}"));
                                    ui.close();
                                }
                                if ui.button(text.get("memory.copy_selected")).clicked() {
                                    let (a, n) = self.selection();
                                    match app.machine.read_memory(a, n) {
                                        Ok(bytes) => ui.ctx().copy_text(hex(&bytes)),
                                        Err(e) => app.report::<(), _>(Err(e)),
                                    }
                                    ui.close();
                                }
                                if ui.button(text.get("memory.copy_pointer")).clicked() {
                                    match app.machine.read_memory(addr, 8) {
                                        Ok(bytes) => ui
                                            .ctx()
                                            .copy_text(format!("0x{:016X}", unsigned(&bytes))),
                                        Err(e) => app.report::<(), _>(Err(e)),
                                    }
                                    ui.close();
                                }
                                if ui.button(text.get("memory.bookmark")).clicked() {
                                    if !self.bookmarks.contains(&addr) {
                                        self.bookmarks.push(addr);
                                    }
                                    ui.close();
                                }
                                if !self.bookmarks.is_empty() {
                                    ui.menu_button(text.get("memory.bookmarks"), |ui| {
                                        for a in self.bookmarks.clone() {
                                            if ui.button(format!("{a:016X}")).clicked() {
                                                self.navigate(a, app);
                                                ui.close();
                                            }
                                        }
                                    });
                                }
                                if ui.button(text.get("memory.inspect_byte")).clicked() {
                                    self.selected = addr;
                                    self.anchor = addr;
                                    self.selection_end = addr;
                                    self.pending_tools.push(Tab::Inspector);
                                    ui.close();
                                }
                                if ui.button(text.get("memory.follow_qword")).clicked() {
                                    self.selected = addr;
                                    self.follow(app);
                                    ui.close();
                                }
                                if ui.button(text.get("memory.watch_u64")).clicked() {
                                    self.selected = addr;
                                    self.add_watch(ValueType::U64);
                                    ui.close();
                                }
                            });
                        }
                    }
                    ui.label(
                        RichText::new(
                            bytes
                                .iter()
                                .map(|b| {
                                    if b.is_ascii_graphic() || *b == b' ' {
                                        *b as char
                                    } else {
                                        '.'
                                    }
                                })
                                .collect::<String>(),
                        )
                        .monospace(),
                    );
                });
            }
        });
        if cancel_edit {
            self.inline_edit = None;
        } else if let Some((address, text, advance, format)) = edit_commit {
            if text.trim().is_empty() {
                self.inline_edit = None;
                return;
            }
            let r = match format {
                MemoryFormat::Bytes => parse_hex(&text),
                MemoryFormat::Qwords => qword_bytes(&text, false),
            }
            .and_then(|bytes| {
                if bytes.is_empty() {
                    Err("enter pairs of hex digits".into())
                } else {
                    app.machine.write_memory(address, &bytes)
                }
            });
            if r.is_ok() {
                let count = match format {
                    MemoryFormat::Bytes => parse_hex(&text).expect("already validated").len(),
                    MemoryFormat::Qwords => 8,
                } as u64;
                self.anchor = address;
                self.selection_end = address + count - 1;
                self.inline_edit = None;
                if advance
                    && let Some(next) = address.checked_add(count).filter(|next| *next < base + len)
                {
                    self.selected = next;
                    self.anchor = next;
                    self.selection_end = next;
                    self.jump = format!("0x{next:016X}");
                    self.inline_edit = Some((next, String::new()));
                    if (next - base) / row_bytes != (address - base) / row_bytes {
                        self.scroll_to = Some(((next - base) / row_bytes) as f32 * 32.0);
                    }
                    let edit_kind = match format {
                        MemoryFormat::Bytes => "byte_edit",
                        MemoryFormat::Qwords => "qword_edit",
                    };
                    ui.memory_mut(|m| m.request_focus(egui::Id::new((edit_kind, next))));
                }
            } else {
                let edit_kind = match format {
                    MemoryFormat::Bytes => "byte_edit",
                    MemoryFormat::Qwords => "qword_edit",
                };
                ui.memory_mut(|m| m.request_focus(egui::Id::new((edit_kind, address))));
            }
            app.report(r);
        }
    }
    fn first_scan(&mut self, app: &mut CookieApp) -> Result<(), String> {
        let result = match self.scan_format {
            ScanFormat::Number => {
                let exact = if self.scan_unknown {
                    None
                } else {
                    Some(parse_u64(&self.scan_value)?)
                };
                self.scanner
                    .first(&app.machine, self.scan_width, exact, self.scan_aligned)
            }
            kind => self
                .scanner
                .first_bytes(&app.machine, &kind.bytes(&self.scan_value)?),
        };
        if result.is_ok() {
            self.result_format = self.scan_format;
            self.result_page = 0;
        }
        result
    }
    fn narrow_scan(&mut self, app: &mut CookieApp) -> Result<(), String> {
        if self.scan_format != self.result_format {
            return Err("Input format changed. Start a new first scan.".into());
        }
        if self.scanner.byte_pattern {
            let target = if self.scan_filter.needs_value() {
                Some(self.scan_format.bytes(&self.scan_value)?)
            } else {
                None
            };
            self.scanner
                .filter_bytes(&app.machine, self.scan_filter, target.as_deref())
        } else {
            let target = if self.scan_filter.needs_value() {
                Some(parse_u64(&self.scan_value)?)
            } else {
                None
            };
            self.scanner.filter(&app.machine, self.scan_filter, target)
        }
    }
    fn scanner_ui(&mut self, ui: &mut egui::Ui, app: &mut CookieApp) {
        let text = app.resources.text.clone();
        ui.horizontal(|ui| {
            ui.label(text.get("scanner.find"));
            egui::ComboBox::from_id_salt("scan_format")
                .selected_text(text.get(self.scan_format.text_key()))
                .show_ui(ui, |ui| {
                    for kind in [ScanFormat::Number, ScanFormat::Text, ScanFormat::HexBytes] {
                        ui.selectable_value(&mut self.scan_format, kind, text.get(kind.text_key()));
                    }
                });
        });
        let hint = match self.scan_format {
            ScanFormat::Number => text.get("scanner.hint.number"),
            ScanFormat::Text => text.get("scanner.hint.text"),
            ScanFormat::HexBytes => text.get("scanner.hint.hex"),
        };
        ui.add(
            egui::TextEdit::singleline(&mut self.scan_value)
                .hint_text(hint)
                .desired_width(f32::INFINITY),
        );
        ui.small(match self.scan_format {
            ScanFormat::Number => text.get("scanner.help.number"),
            ScanFormat::Text => text.get("scanner.help.text"),
            ScanFormat::HexBytes => text.get("scanner.help.hex"),
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button(text.get("scanner.first")).clicked() {
                let r = self.first_scan(app);
                app.report(r);
            }
            if ui
                .add_enabled(
                    self.scanner.initialized,
                    Button::new(text.get("scanner.next")),
                )
                .clicked()
            {
                let r = self.narrow_scan(app);
                app.report(r);
            }
            if ui.button(text.get("scanner.reset")).clicked() {
                self.scanner = Scanner::default();
            }
        });
        ui.collapsing(text.get("scanner.options"), |ui| {
            if self.scan_format == ScanFormat::Number {
                egui::ComboBox::from_id_salt("scan_width")
                    .selected_text(text.format(
                        "scanner.width",
                        &[("bits", (self.scan_width * 8).to_string())],
                    ))
                    .show_ui(ui, |ui| {
                        for width in [1, 2, 4, 8] {
                            ui.selectable_value(
                                &mut self.scan_width,
                                width,
                                text.format("scanner.width", &[("bits", (width * 8).to_string())]),
                            );
                        }
                    });
                ui.checkbox(&mut self.scan_aligned, text.get("scanner.aligned"));
                ui.checkbox(&mut self.scan_unknown, text.get("scanner.unknown"));
            }
            egui::ComboBox::from_id_salt("scan_filter")
                .selected_text(self.scan_filter.label())
                .show_ui(ui, |ui| {
                    for f in ScanFilter::ALL {
                        if self.scan_format == ScanFormat::Number
                            || matches!(
                                f,
                                ScanFilter::Exact
                                    | ScanFilter::Equal
                                    | ScanFilter::NotEqual
                                    | ScanFilter::Changed
                                    | ScanFilter::Unchanged
                            )
                        {
                            ui.selectable_value(&mut self.scan_filter, f, f.label());
                        }
                    }
                });
        });
        if !self.scanner.initialized {
            ui.label(RichText::new(text.get("scanner.start_help")).color(app.theme().muted));
            return;
        }
        ui.small(text.get("scanner.narrow_help"));
        ui.label(
            RichText::new(text.format(
                "scanner.matches",
                &[("count", number(self.scanner.results.len() as u64))],
            ))
            .strong(),
        );
        if self.scanner.results.is_empty() {
            ui.label(text.get("scanner.empty"));
            return;
        }
        ScrollArea::both()
            .id_salt("scan_table")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("scan_table_rows")
                    .num_columns(3)
                    .striped(true)
                    .spacing(egui::vec2(16.0, 8.0))
                    .show(ui, |ui| {
                        ui.label(RichText::new(text.get("scanner.address")).strong());
                        ui.label(RichText::new(text.get("scanner.current")).strong());
                        ui.label("");
                        ui.end_row();
                        // Keep the table bounded while scans retain every match. Paging is explicit.
                        let pages = self.scanner.results.len().div_ceil(100);
                        self.result_page = self.result_page.min(pages.saturating_sub(1));
                        let start = self.result_page * 100;
                        let end = (start + 100).min(self.scanner.results.len());
                        for i in start..end {
                            let a = self.scanner.results[i].address;
                            let current = app
                                .machine
                                .read_memory(a, self.scanner.width as u64)
                                .map(|b| self.result_format.display(&b))
                                .unwrap_or_else(|_| text.get("scanner.unreadable").into());
                            if ui
                                .selectable_label(
                                    self.selected == a,
                                    RichText::new(format!("0x{a:016X}")).monospace(),
                                )
                                .clicked()
                            {
                                self.navigate(a, app);
                            }
                            ui.add(egui::Label::new(current.clone()).truncate())
                                .on_hover_text(current);
                            if ui.button(text.get("scanner.inspect")).clicked() {
                                self.navigate(a, app);
                            }
                            ui.end_row();
                        }
                    });
            });
        let pages = self.scanner.results.len().div_ceil(100);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.result_page > 0,
                    Button::new(text.get("scanner.previous_page")),
                )
                .clicked()
            {
                self.result_page -= 1;
            }
            ui.label(text.format(
                "scanner.page",
                &[
                    ("current", (self.result_page + 1).to_string()),
                    ("total", pages.to_string()),
                ],
            ));
            if ui
                .add_enabled(
                    self.result_page + 1 < pages,
                    Button::new(text.get("scanner.next_page")),
                )
                .clicked()
            {
                self.result_page += 1;
            }
        });
    }
    fn inspector_ui(&mut self, ui: &mut egui::Ui, app: &mut CookieApp) {
        let text = app.resources.text.clone();
        let region = app
            .machine
            .memory_regions()
            .iter()
            .find(|region| region.contains(self.selected));
        let allocation = app
            .machine
            .allocations()
            .iter()
            .rev()
            .find(|a| a.live && a.contains(self.selected));
        egui::Grid::new("inspector_location")
            .num_columns(2)
            .spacing(egui::vec2(16.0, 6.0))
            .show(ui, |ui| {
                ui.label(text.get("inspector.address"));
                ui.monospace(format!("0x{:016X}", self.selected));
                ui.end_row();
                ui.label(text.get("inspector.region_label"));
                if let Some(region) = region {
                    ui.label(text.format(
                        "inspector.region",
                        &[
                            ("kind", format!("{:?}", region.kind())),
                            ("permissions", region.permissions().label()),
                        ],
                    ));
                } else {
                    ui.label(text.get("inspector.unmapped"));
                }
                ui.end_row();
                ui.label(text.get("inspector.allocation_label"));
                if let Some(a) = allocation {
                    ui.label(text.format(
                        "inspector.allocation",
                        &[
                            ("id", a.id.to_string()),
                            ("offset", format!("{:X}", self.selected - a.base)),
                        ],
                    ));
                } else {
                    ui.label(pointer_label(self.selected, app.machine.allocations()));
                }
                ui.end_row();
                if let Some(a) = allocation {
                    ui.label(text.get("inspector.size"));
                    ui.label(text.format(
                        "inspector.allocation_detail",
                        &[
                            ("size", number(a.size)),
                            ("generation", a.generation.to_string()),
                        ],
                    ));
                    ui.end_row();
                    ui.label(text.get("inspector.range"))
                        .on_hover_text(text.get("inspector.range_hint"));
                    ui.vertical(|ui| {
                        ui.monospace(format!("0x{:016X}", a.base));
                        ui.monospace(
                            text.format(
                                "inspector.to",
                                &[("address", format!("{:016X}", a.end()))],
                            ),
                        );
                    });
                    ui.end_row();
                    if self.symbols {
                        ui.label(text.get("inspector.object"));
                        ui.label(&a.semantic_type);
                        ui.end_row();
                    }
                }
            });
        ui.checkbox(&mut self.symbols, text.get("inspector.show_objects"))
            .on_hover_text(text.get("inspector.show_objects_hint"));
        if ui.button(text.get("inspector.follow")).clicked() {
            self.follow(app);
        }
        ui.add_space(8.0);
        ui.separator();
        egui::Grid::new("typed_interpretations")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label(RichText::new(text.get("inspector.type")).strong());
                ui.label(RichText::new(text.get("inspector.value")).strong());
                ui.end_row();
                for kind in ValueType::ALL {
                    ui.label(kind.label());
                    match app.machine.read_memory(self.selected, kind.width() as u64) {
                        Ok(b) => {
                            ui.label(kind.interpret(&b).unwrap_or_else(|e| e));
                        }
                        Err(_) => {
                            ui.label(text.get("inspector.unreadable"));
                        }
                    }
                    ui.end_row();
                }
            });
        ui.add_space(16.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.watch_name)
                .hint_text(text.get("watch.label_hint")),
        );
        if ui.button(text.get("watch.add")).clicked() {
            self.add_watch(ValueType::U64);
        }
    }
    fn watch_ui(&mut self, ui: &mut egui::Ui, app: &mut CookieApp) {
        let text = app.resources.text.clone();
        if self.watches.is_empty() {
            ui.label(text.get("watch.empty"));
        }
        let mut remove = None;
        ScrollArea::both().id_salt("watches").show(ui, |ui| {
            for (i, w) in self.watches.clone().iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.button(text.get("watch.remove")).clicked() {
                        remove = Some(i);
                    }
                    if ui
                        .selectable_label(
                            self.selected == w.address,
                            RichText::new(format!("{:016X}", w.address)).monospace(),
                        )
                        .clicked()
                    {
                        self.navigate(w.address, app);
                    }
                });
                ui.label(text.format(
                    "watch.row",
                    &[
                        ("address", format!("{:016X}", w.address)),
                        ("name", w.name.clone()),
                        (
                            "value",
                            format!(
                                "{} · {}",
                                w.kind.label(),
                                w.value(&app.machine).unwrap_or_else(|e| e)
                            ),
                        ),
                    ],
                ));
                ui.add_space(8.0);
            }
        });
        if let Some(i) = remove {
            self.watches.remove(i);
        }
    }
    fn history_ui(&mut self, ui: &mut egui::Ui, app: &mut CookieApp) {
        let text = app.resources.text.clone();
        ui.label(text.get("history.heading"));
        if app.machine.memory.events().is_empty() {
            ui.label(text.get("history.empty"));
        }
        let events = app
            .machine
            .memory
            .events()
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>();
        ScrollArea::both().id_salt("write_history").show(ui, |ui| {
            for (i, e) in events.iter().enumerate() {
                ui.push_id(i, |ui| {
                    let overflow = if e.overflow > 0 {
                        text.format(
                            "history.allocation_overflow",
                            &[("count", e.overflow.to_string())],
                        )
                    } else if e.semantic_overflow > 0 {
                        text.format(
                            "history.field_overflow",
                            &[("count", e.semantic_overflow.to_string())],
                        )
                    } else {
                        String::new()
                    };
                    let title = text.format(
                        "history.title",
                        &[
                            ("tick", e.tick.to_string()),
                            ("source", format!("{:?}", e.source)),
                            ("operation", e.operation.clone()),
                            ("count", e.new.len().to_string()),
                            ("overflow", overflow),
                        ],
                    );
                    egui::CollapsingHeader::new(RichText::new(title).color(
                        if e.overflow > 0 || e.semantic_overflow > 0 {
                            app.theme().danger
                        } else {
                            app.theme().text
                        },
                    ))
                    .show(ui, |ui| {
                        if ui
                            .button(text.format(
                                "history.jump",
                                &[("address", format!("{:016X}", e.address))],
                            ))
                            .clicked()
                        {
                            self.navigate(e.address, app);
                        }
                        if let Some(boundary) = e.semantic_boundary {
                            ui.monospace(text.format(
                                "history.field_end",
                                &[
                                    ("address", format!("{boundary:016X}")),
                                    ("count", e.semantic_overflow.to_string()),
                                ],
                            ));
                        }
                        ui.label(text.format(
                            "history.origin",
                            &[
                                ("origin", format!("{:?}", e.origin)),
                                ("affected", format!("{:?}", e.affected)),
                            ],
                        ));
                        if let Some(id) = e.origin
                            && let Some(a) = app.machine.allocations().iter().find(|a| a.id == id)
                        {
                            ui.monospace(text.format(
                                "history.intended",
                                &[
                                    ("start", format!("{:016X}", a.base)),
                                    ("end", format!("{:016X}", a.end())),
                                ],
                            ));
                        }
                        ui.monospace(text.format(
                            "history.old",
                            &[("bytes", hex(&e.old[..e.old.len().min(128)]))],
                        ));
                        ui.monospace(text.format(
                            "history.new",
                            &[("bytes", hex(&e.new[..e.new.len().min(128)]))],
                        ));
                        if e.new.len() > 128 {
                            ui.small(text.get("history.preview"));
                        }
                    });
                });
            }
        });
    }
    fn diff_ui(&mut self, ui: &mut egui::Ui, app: &mut CookieApp) {
        let text = app.resources.text.clone();
        ui.horizontal(|ui| {
            if ui.button(text.get("diff.take")).clicked() {
                self.snapshot = Some(app.machine.memory.snapshot());
                self.diffs.clear();
            }
            if ui
                .add_enabled(
                    self.snapshot.is_some(),
                    Button::new(text.get("diff.compare")),
                )
                .clicked()
            {
                self.diffs = diff(self.snapshot.as_ref().unwrap(), &app.machine);
            }
        });
        if self.snapshot.is_none() {
            ui.label(text.get("diff.help"));
            return;
        }
        ui.label(text.format("diff.count", &[("count", number(self.diffs.len() as u64))]));
        ScrollArea::both().id_salt("diff_rows").show_rows(
            ui,
            32.0,
            self.diffs.len(),
            |ui, range| {
                for i in range {
                    let d = &self.diffs[i];
                    let a = d.address;
                    let label = format!("{a:016X}    {:02X} → {:02X}", d.before, d.after);
                    if ui
                        .selectable_label(self.selected == a, RichText::new(label).monospace())
                        .clicked()
                    {
                        self.navigate(a, app);
                    }
                }
            },
        );
    }
}
struct Viewer<'a> {
    workspace: &'a mut DebugWorkspace,
    app: &'a mut CookieApp,
}
impl TabViewer for Viewer<'_> {
    type Tab = Tab;
    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::new(*tab)
    }
    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        self.app.text(tab.text_key()).into()
    }
    fn is_closeable(&self, _: &Tab) -> bool {
        true
    }
    fn scroll_bars(&self, _: &Tab) -> [bool; 2] {
        [false, false]
    }
    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        egui::Frame::NONE
            .inner_margin(8.0)
            .show(ui, |ui| match tab {
                Tab::Memory => self.workspace.memory_ui(ui, self.app),
                Tab::Scanner => self.workspace.scanner_ui(ui, self.app),
                Tab::Inspector => {
                    ScrollArea::vertical().id_salt("inspector").show(ui, |ui| {
                        self.workspace.inspector_ui(ui, self.app);
                    });
                }
                Tab::Watch => self.workspace.watch_ui(ui, self.app),
                Tab::History => self.workspace.history_ui(ui, self.app),
                Tab::Diff => self.workspace.diff_ui(ui, self.app),
            });
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
fn qword_bytes(text: &str, require_full_width: bool) -> Result<Vec<u8>, String> {
    let digits = text.trim().strip_prefix("0x").unwrap_or(text.trim());
    if digits.is_empty() || digits.len() > 16 || (require_full_width && digits.len() != 16) {
        return Err("enter up to 16 hexadecimal digits".into());
    }
    let value = u64::from_str_radix(digits, 16)
        .map_err(|_| "qword contains a non-hexadecimal digit".to_string())?;
    Ok(value.to_le_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(ctx: &egui::Context, app: &mut CookieApp, events: Vec<egui::Event>) {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100.0, 850.0),
                )),
                events,
                ..Default::default()
            },
            |ui| app.show(ui),
        )
        .drop_without_applying_deltas();
    }
    #[test]
    fn starts_without_tools_and_opens_only_floating_tools() {
        let ctx = egui::Context::default();
        let mut app = CookieApp::headless(cookie_game::Machine::new(1).unwrap(), true);
        frame(&ctx, &mut app, vec![]);
        let workspace = app.debugger.as_mut().unwrap();
        assert_eq!(workspace.dock.iter_all_tabs().count(), 0);
        workspace.open_tool(Tab::Scanner, &ctx);
        assert!(workspace.dock.main_surface().is_empty());
        assert_eq!(workspace.dock.iter_all_tabs().count(), 1);
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        assert_eq!(
            app.debugger.as_ref().unwrap().dock.iter_all_tabs().count(),
            1
        );
    }
    #[test]
    fn inline_hex_edit_writes_guest_bytes() {
        let ctx = egui::Context::default();
        let mut app = CookieApp::headless(cookie_game::Machine::new(1).unwrap(), true);
        app.theme().apply(&ctx, false);
        frame(&ctx, &mut app, vec![]);
        let address = app.machine.address(cookie_game::field::COOKIES).unwrap();
        let mut workspace = app.debugger.take().unwrap();
        workspace.navigate(address, &mut app);
        workspace.open_tool(Tab::Memory, &ctx);
        workspace.inline_edit = Some((address, String::new()));
        app.debugger = Some(workspace);
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        ctx.memory_mut(|m| m.request_focus(egui::Id::new(("byte_edit", address))));
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Text("E8 03 00 00 00 00 00 00".into())],
        );
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(
            app.machine.value(cookie_game::field::COOKIES).unwrap(),
            1000
        );
        assert_eq!(
            app.machine.memory.events().back().unwrap().source,
            vm_core::WriteSource::Debugger
        );
    }
    fn begin_edit(ctx: &egui::Context, app: &mut CookieApp, address: Address) {
        let mut workspace = app.debugger.take().unwrap();
        workspace.navigate(address, app);
        workspace.open_tool(Tab::Memory, ctx);
        workspace.inline_edit = Some((address, String::new()));
        app.debugger = Some(workspace);
        frame(ctx, app, vec![]);
        frame(ctx, app, vec![]);
        ctx.memory_mut(|m| m.request_focus(egui::Id::new(("byte_edit", address))));
    }
    #[test]
    fn hex_pairs_advance_across_rows_and_stop_at_heap_end() {
        let ctx = egui::Context::default();
        let mut app = CookieApp::headless(cookie_game::Machine::new(1).unwrap(), true);
        app.theme().apply(&ctx, false);
        frame(&ctx, &mut app, vec![]);
        let address = Architecture::HEAP + 0x100F;
        begin_edit(&ctx, &mut app, address);
        frame(&ctx, &mut app, vec![egui::Event::Text("E".into())]);
        assert_eq!(app.machine.read_memory(address, 1).unwrap(), vec![0]);
        frame(&ctx, &mut app, vec![egui::Event::Text("8".into())]);
        assert_eq!(app.debugger.as_ref().unwrap().selected, address + 1);
        frame(&ctx, &mut app, vec![egui::Event::Text("0".into())]);
        frame(&ctx, &mut app, vec![egui::Event::Text("3".into())]);
        assert_eq!(app.machine.read_memory(address, 2).unwrap(), vec![0xE8, 3]);
        assert_eq!(app.debugger.as_ref().unwrap().selected, address + 2);
        let end = Architecture::HEAP + Architecture::HEAP_SIZE - 1;
        begin_edit(&ctx, &mut app, end);
        frame(&ctx, &mut app, vec![egui::Event::Text("AA BB".into())]);
        assert!(app.error);
        assert_eq!(app.machine.read_memory(end, 1).unwrap(), vec![0]);
        begin_edit(&ctx, &mut app, end);
        frame(&ctx, &mut app, vec![egui::Event::Text("FF".into())]);
        assert_eq!(app.machine.read_memory(end, 1).unwrap(), vec![255]);
        assert!(app.debugger.as_ref().unwrap().inline_edit.is_none());
        assert!(!app.error);
    }
    #[test]
    fn qword_mode_displays_and_writes_little_endian_values() {
        let ctx = egui::Context::default();
        let mut app = CookieApp::headless(cookie_game::Machine::new(1).unwrap(), true);
        app.theme().apply(&ctx, false);
        frame(&ctx, &mut app, vec![]);
        let address = Architecture::HEAP;
        app.machine
            .write_memory(address, &0x1122_3344_5566_7788u64.to_le_bytes())
            .unwrap();
        let mut workspace = app.debugger.take().unwrap();
        workspace.memory_format = MemoryFormat::Qwords;
        workspace.open_tool(Tab::Memory, &ctx);
        workspace.inline_edit = Some((address, String::new()));
        app.debugger = Some(workspace);
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        ctx.memory_mut(|memory| memory.request_focus(egui::Id::new(("qword_edit", address))));
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Text("0102030405060708".into())],
        );
        assert_eq!(
            app.machine.read_memory(address, 8).unwrap(),
            vec![0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
        );
        assert_eq!(app.debugger.as_ref().unwrap().selected, address + 8);
        assert_eq!(
            qword_bytes("0x1122334455667788", false).unwrap(),
            vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]
        );
    }
    #[test]
    fn memory_shows_heap_and_code_and_opens_a_read_only_inspector() {
        let ctx = egui::Context::default();
        let mut app = CookieApp::headless(cookie_game::Machine::new(1).unwrap(), true);
        app.theme().apply(&ctx, false);
        frame(&ctx, &mut app, vec![]);
        let mut workspace = app.debugger.take().unwrap();
        workspace.open_tool(Tab::Memory, &ctx);
        workspace.navigate(Architecture::CODE, &mut app);
        assert_eq!(workspace.selected, Architecture::CODE);
        assert_eq!(workspace.memory_region, RegionKind::Code);
        assert!(!app.error);
        assert!(
            app.machine
                .read_memory(Architecture::CODE + 0x100, 8)
                .is_ok()
        );
        workspace.navigate(Architecture::GLOBALS, &mut app);
        assert_eq!(workspace.selected, Architecture::CODE);
        assert!(app.error);
        workspace.navigate(Architecture::HEAP, &mut app);
        app.debugger = Some(workspace);
        crate::tests::click_label(&ctx, &mut app, "Inspector");
        assert!(
            app.debugger
                .as_ref()
                .unwrap()
                .dock
                .find_tab(&Tab::Inspector)
                .is_some()
        );
        assert_eq!(
            app.debugger.as_ref().unwrap().dock.iter_all_tabs().count(),
            2
        );
        let output = crate::tests::render(&ctx, &mut app, vec![]);
        let mut painted = vec![];
        for s in &output.shapes {
            crate::tests::texts(&s.shape, &mut painted);
        }
        for removed in ["Write typed value", "More memory actions", "Edit raw bytes"] {
            assert!(!painted.iter().any(|(text, _, _, _)| text == removed));
        }
        output.drop_without_applying_deltas();
    }
    #[test]
    fn scanner_ui_accepts_numbers_text_and_bytes() {
        let mut app = CookieApp::headless(cookie_game::Machine::new(1).unwrap(), true);
        app.machine.click().unwrap();
        let mut w = app.debugger.take().unwrap();
        for input in ["1", "0x1", "0b1"] {
            w.scan_value = input.into();
            w.first_scan(&mut app).unwrap();
            assert!(w.scanner.results.iter().any(|r|r.address==app.machine.address(cookie_game::field::COOKIES).unwrap()));
        }
        w.scan_format = ScanFormat::Text;
        w.scan_value = "Crumb & Co.".into();
        w.first_scan(&mut app).unwrap();
        assert_eq!(w.scanner.results.len(), 1);
        w.scan_format = ScanFormat::HexBytes;
        w.scan_value = "43 72 75 6D 62".into();
        w.first_scan(&mut app).unwrap();
        assert_eq!(w.scanner.results.len(), 1);
    }
}
