//! Native workshop interface with a draggable live memory window.
#![forbid(unsafe_code)]
use cookie_game::{CURSOR_PRICE, FLAG_PRICE, GameView, Machine, globals};
use eframe::egui::{self, Color32, RichText};
use std::time::{Duration, Instant};
use vm_core::{Address, Architecture, ExecutionBackend};

const COPY: &str = include_str!("../../../assets/strings.txt");
fn text(key: &str) -> &'static str {
    COPY.lines()
        .filter_map(|line| line.split_once('='))
        .find_map(|(candidate, value)| (candidate.trim() == key).then_some(value.trim()))
        .unwrap_or("<missing text>")
}

pub fn window_title() -> String {
    text("app.title").into()
}

pub struct CookieApp {
    machine: Machine,
    last_valid: Machine,
    memory_open: bool,
    pointer_mode: bool,
    qwords: bool,
    memory_base: Address,
    jump: String,
    selected: Option<Address>,
    edit_value: String,
    username: String,
    cursor_editor: Option<usize>,
    cursor_name: String,
    cursor_description: String,
    status: String,
    last_frame: Instant,
    accumulator: Duration,
}

impl CookieApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, machine: Machine, _debug: bool) -> Self {
        Self::headless(machine)
    }
    pub fn headless(machine: Machine) -> Self {
        let username = machine.view().map(|v| v.username).unwrap_or_default();
        Self {
            last_valid: machine.clone(),
            machine,
            memory_open: false,
            pointer_mode: false,
            qwords: false,
            memory_base: Architecture::GLOBALS,
            jump: format!("0x{:X}", Architecture::GLOBALS),
            selected: None,
            edit_value: String::new(),
            username,
            cursor_editor: None,
            cursor_name: String::new(),
            cursor_description: String::new(),
            status: String::new(),
            last_frame: Instant::now(),
            accumulator: Duration::ZERO,
        }
    }
    fn report(&mut self, result: Result<(), impl std::fmt::Display>) {
        self.status = result.err().map(|e| e.to_string()).unwrap_or_default();
    }
    fn tick(&mut self) {
        let now = Instant::now();
        self.accumulator += now
            .duration_since(self.last_frame)
            .min(Duration::from_millis(250));
        self.last_frame = now;
        while self.accumulator >= Duration::from_millis(50) {
            self.accumulator -= Duration::from_millis(50);
            if let Err(e) = self.machine.advance_tick() {
                self.status = e.to_string();
                break;
            }
        }
    }
    fn safe_view(&mut self) -> Option<GameView> {
        match self.machine.view() {
            Ok(v) => {
                self.last_valid = self.machine.clone();
                Some(v)
            }
            Err(e) => {
                self.machine = self.last_valid.clone();
                self.status = e.to_string();
                self.machine.view().ok()
            }
        }
    }
    fn jump_to(&mut self, address: Address) {
        self.memory_base = address & !0xFu64;
        self.jump = format!("0x{address:X}");
        self.memory_open = true;
    }
    fn parse_address(text: &str) -> Result<u64, String> {
        let t = text.trim().replace('_', "");
        if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
            u64::from_str_radix(h, 16)
        } else {
            t.parse()
        }
        .map_err(|_| "Enter a hexadecimal or decimal address".into())
    }
    fn parse_field(text: &str) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        let b = text.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'\\' && i + 3 < b.len() && b[i + 1] == b'x' {
                let pair = std::str::from_utf8(&b[i + 2..i + 4]).map_err(|_| "Invalid \\x byte")?;
                out.push(
                    u8::from_str_radix(pair, 16)
                        .map_err(|_| "Use \\x followed by two hexadecimal digits")?,
                );
                i += 4;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        Ok(out)
    }
    fn pointer_target(&self, address: Address) -> Option<Address> {
        let b = self.machine.read_memory(address, 8).ok()?;
        let a = u64::from_le_bytes(b.try_into().ok()?);
        self.machine
            .memory_regions()
            .iter()
            .any(|r| r.contains(a))
            .then_some(a)
    }
    fn field_info(&self, address: Address) -> Option<String> {
        let g = Architecture::GLOBALS;
        let globals = [
            (g + globals::COOKIES, "Cookies*", "pointer"),
            (g + globals::USER, "User*", "pointer"),
            (g + globals::CURSORS, "Cursor**", "pointer"),
            (g + globals::CURSOR_COUNT, "cursor_count", "u64"),
            (g + globals::MAX_FIELD_SIZE, "max_field_size", "u64"),
            (g + globals::FUNCTIONS, "functions", "pointer"),
        ];
        for (a, n, t) in globals {
            if address >= a && address < a + 8 {
                let value = self.machine.memory.read_u64(a).ok()?;
                return Some(format!(
                    "{n}\n{t}\nvalue 0x{value:016X}\naddress 0x{a:016X}"
                ));
            }
        }
        for (a, name) in [
            (cookie_game::FUNCTION_TABLE, "add_cookie"),
            (cookie_game::FUNCTION_TABLE + 8, "win"),
        ] {
            if address >= a && address < a + 8 {
                let value = self.machine.memory.read_u64(a).ok()?;
                return Some(format!("{name}\nfunction pointer\nvalue 0x{value:016X}"));
            }
        }
        for a in self.machine.allocations() {
            if address >= a.base && address < a.base + a.size {
                let offset = address - a.base;
                let (name, ty) = match a.semantic_type.as_str() {
                    "struct Cookies" => ("cookies", "u64"),
                    "struct User" if offset < 8 => ("id", "u64"),
                    "struct User" => ("username", "char*"),
                    "struct Cursor" if offset < 8 => ("id", "u64"),
                    "struct Cursor" if offset < 16 => ("name", "char[8]"),
                    "struct Cursor" if offset < 24 => ("description", "char*"),
                    "struct Cursor" => ("on_clicked", "function pointer"),
                    _ => (a.semantic_type.as_str(), "bytes"),
                };
                let field_base = a.base + (offset / 8) * 8;
                let value = self
                    .machine
                    .memory
                    .read_u64(field_base)
                    .ok()
                    .map(|v| format!("0x{v:016X}"))
                    .unwrap_or_else(|| "unreadable".into());
                return Some(format!(
                    "{name}\n{ty}\nvalue {value}\n{} +0x{:X}\naddress 0x{address:016X}",
                    a.semantic_type, offset
                ));
            }
        }
        None
    }
    fn field_color(&self, address: Address) -> Option<Color32> {
        for allocation in self.machine.allocations() {
            if address >= allocation.base && address < allocation.base + allocation.size {
                return Some(match ((address - allocation.base) / 8) % 4 {
                    0 => Color32::LIGHT_BLUE,
                    1 => Color32::LIGHT_GREEN,
                    2 => Color32::YELLOW,
                    _ => Color32::LIGHT_RED,
                });
            }
        }
        None
    }
    fn is_pointer_field(&self, address: Address) -> bool {
        let g = Architecture::GLOBALS;
        if [0, 8, 16, 40]
            .iter()
            .any(|o| address >= g + o && address < g + o + 8)
        {
            return true;
        }
        for a in self.machine.allocations() {
            if a.semantic_type == "struct User" && address >= a.base + 8 && address < a.base + 16 {
                return true;
            }
            if a.semantic_type == "struct Cursor" && address >= a.base + 16 && address < a.base + 32
            {
                return true;
            }
        }
        false
    }
    fn memory_window(&mut self, ctx: &egui::Context) {
        if !self.memory_open {
            return;
        }
        let mut open = true;
        egui::Window::new(text("memory.title"))
            .id(egui::Id::new("memory_window"))
            .open(&mut open)
            .default_size([820.0, 560.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .selectable_label(!self.qwords, text("memory.bytes"))
                        .clicked()
                    {
                        self.qwords = false;
                    }
                    if ui
                        .selectable_label(self.qwords, text("memory.qwords"))
                        .clicked()
                    {
                        self.qwords = true;
                    }
                    ui.separator();
                    ui.label(text("memory.address"));
                    ui.text_edit_singleline(&mut self.jump);
                    if ui.button(text("memory.jump")).clicked() {
                        match Self::parse_address(&self.jump) {
                            Ok(a) => self.memory_base = a & !0xFu64,
                            Err(e) => self.status = e,
                        }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    for (label, a) in [
                        (".text", Architecture::CODE),
                        (".bss", Architecture::GLOBALS),
                        ("heap", Architecture::HEAP),
                        ("functions", cookie_game::FUNCTION_TABLE),
                    ] {
                        if ui.button(label).clicked() {
                            self.jump_to(a);
                        }
                    }
                });
                if self.memory_base == cookie_game::FUNCTION_TABLE {
                    ui.separator();
                    egui::Grid::new("function_table")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong(text("functions.name"));
                            ui.strong(text("functions.address"));
                            ui.end_row();
                            match self.machine.function_table() {
                                Ok(functions) => {
                                    for function in functions {
                                        let name = match function.name {
                                            "add_cookie" => text("functions.add_cookie"),
                                            "win" => text("functions.win"),
                                            _ => function.name,
                                        };
                                        ui.monospace(name);
                                        let address = function.address;
                                        if ui
                                            .link(
                                                RichText::new(format!("0x{address:016X}"))
                                                    .monospace()
                                                    .color(Color32::RED),
                                            )
                                            .clicked()
                                        {
                                            self.jump_to(address);
                                        }
                                        ui.end_row();
                                    }
                                }
                                Err(_) => {
                                    ui.label(text("memory.unmapped"));
                                    ui.end_row();
                                }
                            }
                        });
                }
                if let Some(address) = self.selected {
                    ui.horizontal(|ui| {
                        ui.label(format!("0x{address:016X}"));
                        ui.text_edit_singleline(&mut self.edit_value);
                        if ui.button(text("memory.write")).clicked() {
                            let parsed = if self.qwords {
                                Self::parse_address(&self.edit_value)
                                    .map(|v| v.to_le_bytes().to_vec())
                            } else {
                                u8::from_str_radix(
                                    self.edit_value.trim().trim_start_matches("0x"),
                                    16,
                                )
                                .map(|v| vec![v])
                                .map_err(|_| "Enter one hexadecimal byte".into())
                            };
                            match parsed {
                                Ok(bytes) => {
                                    let result = self.machine.write_memory(address, &bytes);
                                    self.report(result)
                                }
                                Err(e) => self.status = e,
                            }
                        }
                    });
                }
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for row in 0..32u64 {
                        let base = self.memory_base + row * 16;
                        let bytes = self.machine.read_memory(base, 16);
                        ui.horizontal(|ui| {
                            ui.monospace(format!("{base:016X}"));
                            match bytes {
                                Ok(bytes) => {
                                    if self.qwords {
                                        for q in 0..2 {
                                            let a = base + q * 8;
                                            let v = u64::from_le_bytes(
                                                bytes[q as usize * 8..q as usize * 8 + 8]
                                                    .try_into()
                                                    .unwrap(),
                                            );
                                            let text = format!("{v:016X}");
                                            let mut rt = RichText::new(text).monospace();
                                            if self.is_pointer_field(a) {
                                                rt = rt.color(Color32::RED);
                                            }
                                            let response =
                                                ui.selectable_label(self.selected == Some(a), rt);
                                            let response = if let Some(info) = self.field_info(a) {
                                                response.on_hover_text(info)
                                            } else {
                                                response
                                            };
                                            if response.clicked() {
                                                self.selected = Some(a);
                                                self.edit_value = format!("0x{v:X}");
                                            }
                                            if response.double_clicked()
                                                && let Some(p) = self.pointer_target(a)
                                            {
                                                self.jump_to(p);
                                            }
                                        }
                                    } else {
                                        for (i, v) in bytes.iter().enumerate() {
                                            let a = base + i as u64;
                                            let mut rt =
                                                RichText::new(format!("{v:02X}")).monospace();
                                            if self.is_pointer_field(a) {
                                                rt = rt.color(Color32::RED);
                                            } else if let Some(color) = self.field_color(a) {
                                                rt = rt.underline().color(color);
                                            }
                                            let response =
                                                ui.selectable_label(self.selected == Some(a), rt);
                                            let response = if let Some(info) = self.field_info(a) {
                                                response.on_hover_text(info)
                                            } else {
                                                response
                                            };
                                            if response.clicked() {
                                                self.selected = Some(a);
                                                self.edit_value = format!("{v:02X}");
                                            }
                                        }
                                    }
                                    let ascii: String = bytes
                                        .iter()
                                        .map(|b| {
                                            if b.is_ascii_graphic() {
                                                *b as char
                                            } else {
                                                '.'
                                            }
                                        })
                                        .collect();
                                    ui.monospace(ascii);
                                }
                                Err(_) => {
                                    ui.label(text("memory.unmapped"));
                                }
                            }
                        });
                    }
                });
                ui.separator();
            });
        self.memory_open = open;
    }
    fn game(&mut self, ui: &mut egui::Ui, view: &GameView) {
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .selectable_label(self.pointer_mode, text("pointer.toggle"))
                    .clicked()
                {
                    self.pointer_mode = !self.pointer_mode;
                }
                if ui.button(text("memory.open")).clicked() {
                    self.memory_open = true;
                }
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            let response = ui.label(format!("{} {}", text("game.user"), view.username));
            if self.pointer_mode {
                response.on_hover_text(format!("username → 0x{:016X}", view.username_ptr));
                if ui
                    .link(RichText::new(format!("0x{:016X}", view.username_ptr)).monospace())
                    .clicked()
                {
                    self.jump_to(view.username_ptr);
                }
            }
            ui.text_edit_singleline(&mut self.username);
            if ui.button(text("game.username.change")).clicked() {
                match Self::parse_field(&self.username) {
                    Ok(v) => {
                        let result = self.machine.rename_user(&v);
                        self.report(result);
                    }
                    Err(e) => self.status = e,
                }
            }
        });
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.heading(format!("{} {}", view.cookies, text("game.cookies")));
            let cookie = ui.add_sized([180.0, 180.0], egui::Button::new(text("game.cookie")));
            if self.pointer_mode {
                cookie
                    .clone()
                    .on_hover_text(format!("Cookies.cookies → 0x{:016X}", view.cookies_ptr));
            }
            if cookie.clicked() {
                let r = self.machine.click();
                self.report(r);
            }
            if self.pointer_mode
                && ui
                    .link(RichText::new(format!("0x{:016X}", view.cookies_ptr)).monospace())
                    .clicked()
            {
                self.jump_to(view.cookies_ptr);
            }
        });
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    view.cookies >= CURSOR_PRICE,
                    egui::Button::new(format!("{} — {CURSOR_PRICE}", text("game.cursor.buy"))),
                )
                .clicked()
            {
                let r = self.machine.buy_cursor();
                self.report(r);
            }
            if ui
                .add_enabled(
                    view.cookies >= FLAG_PRICE,
                    egui::Button::new(format!("{} — {FLAG_PRICE}", text("game.flag.buy"))),
                )
                .clicked()
            {
                let r = self.machine.buy_flag();
                self.report(r);
            }
        });
        ui.separator();
        ui.heading(text("game.cursors"));
        for (i, c) in view.cursors.iter().enumerate() {
            ui.horizontal(|ui| {
                let response = ui.label(format!("{} #{}", c.name, c.id));
                if self.pointer_mode {
                    response.on_hover_text(format!("Cursor → 0x{:016X}", c.address));
                    if ui
                        .link(RichText::new(format!("0x{:016X}", c.address)).monospace())
                        .clicked()
                    {
                        self.jump_to(c.address);
                    }
                }
                ui.label(&c.description);
                if ui.button(text("game.cursor.click")).clicked() {
                    let r = self.machine.invoke_cursor(i);
                    self.report(r);
                }
                if ui.button(text("game.cursor.edit")).clicked() {
                    self.cursor_editor = Some(i);
                    self.cursor_name = c.name.clone();
                    self.cursor_description = c.description.clone();
                }
            });
        }
        if view.won {
            ui.separator();
            ui.heading(text("game.win.title"));
        }
        if !self.status.is_empty() {
            ui.separator();
            ui.label(&self.status);
        }
    }
    fn edit_cursor_window(&mut self, ctx: &egui::Context) {
        let Some(i) = self.cursor_editor else { return };
        let mut open = true;
        egui::Window::new(text("cursor.edit.title"))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(text("cursor.name"));
                ui.text_edit_singleline(&mut self.cursor_name);
                if ui.button(text("cursor.name.update")).clicked() {
                    match Self::parse_field(&self.cursor_name) {
                        Ok(v) => {
                            let result = self.machine.rename_cursor(i, &v);
                            self.report(result)
                        }
                        Err(e) => self.status = e,
                    }
                }
                ui.label(text("cursor.description"));
                ui.text_edit_multiline(&mut self.cursor_description);
                if ui.button(text("cursor.description.update")).clicked() {
                    match Self::parse_field(&self.cursor_description) {
                        Ok(v) => {
                            let result = self.machine.describe_cursor(i, &v);
                            self.report(result)
                        }
                        Err(e) => self.status = e,
                    }
                }
            });
        if !open {
            self.cursor_editor = None;
        }
    }
}
impl eframe::App for CookieApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.tick();
        if let Some(view) = self.safe_view() {
            self.game(ui, &view);
        }
        self.memory_window(&ctx);
        self.edit_cursor_window(&ctx);
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_byte_notation() {
        assert_eq!(
            CookieApp::parse_field("A\\x00\\xFF").unwrap(),
            vec![65, 0, 255]
        );
        assert!(CookieApp::parse_field("\\xGG").is_err());
    }
    #[test]
    fn debugger_is_built_in() {
        let app = CookieApp::headless(Machine::new(1).unwrap());
        assert_eq!(app.memory_base, Architecture::GLOBALS);
    }
    #[test]
    fn function_addresses_are_distinct() {
        assert_ne!(cookie_game::FN_ADD_COOKIE, cookie_game::FN_WIN);
    }
    #[test]
    fn invalid_pointer_restores_last_valid_state() {
        let mut app = CookieApp::headless(Machine::new(1).unwrap());
        app.machine
            .write_memory(
                Architecture::GLOBALS + globals::USER,
                &u64::MAX.to_le_bytes(),
            )
            .unwrap();
        let view = app.safe_view().unwrap();
        assert_eq!(view.username, "student");
        assert!(app.status.contains("segmentation fault"));
    }
}
