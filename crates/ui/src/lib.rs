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
    fn invoke_cursor(&mut self, index: usize) {
        match self.machine.invoke_cursor(index) {
            Ok(()) => self.status.clear(),
            Err(error) => {
                self.machine = self.last_valid.clone();
                self.status = error.to_string();
            }
        }
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
    fn parse_address(input: &str) -> Result<u64, String> {
        let t = input.trim().replace('_', "");
        if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
            u64::from_str_radix(h, 16)
        } else {
            t.parse()
        }
        .map_err(|_| text("error.address").into())
    }
    fn parse_field(input: &str) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        let b = input.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'\\' && i + 3 < b.len() && b[i + 1] == b'x' {
                let pair =
                    std::str::from_utf8(&b[i + 2..i + 4]).map_err(|_| text("error.raw_byte"))?;
                out.push(u8::from_str_radix(pair, 16).map_err(|_| text("error.raw_byte_format"))?);
                i += 4;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        Ok(out)
    }
    fn pointer_target(&self, address: Address) -> Option<Address> {
        let address = self.pointer_field_base(address).unwrap_or(address);
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
            (
                g + globals::COOKIES,
                text("schema.cookies_pointer"),
                text("schema.pointer"),
            ),
            (
                g + globals::USER,
                text("schema.user_pointer"),
                text("schema.pointer"),
            ),
            (
                g + globals::CURSORS,
                text("schema.cursor_array_pointer"),
                text("schema.pointer"),
            ),
            (
                g + globals::CURSOR_COUNT,
                text("schema.cursor_count"),
                text("schema.u64"),
            ),
            (
                g + globals::MAX_FIELD_SIZE,
                text("schema.max_field_size"),
                text("schema.u64"),
            ),
            (
                g + globals::FUNCTIONS,
                text("schema.functions"),
                text("schema.pointer"),
            ),
            (g + globals::WON, text("schema.won"), text("schema.u64")),
            (
                g + globals::FLAG_OWNED,
                text("schema.flag_owned"),
                text("schema.u64"),
            ),
        ];
        for (a, n, t) in globals {
            if address >= a && address < a + 8 {
                let value = self.machine.memory.read_u64(a).ok()?;
                let mut info = format!(
                    "{n}\n{t}\n{} 0x{value:016X}\n{} 0x{a:016X}",
                    text("schema.value"),
                    text("schema.address")
                );
                self.append_pointer_preview(&mut info, address);
                return Some(info);
            }
        }
        let cursor_table = g + globals::CURSOR_TABLE;
        if address >= cursor_table && address < cursor_table + cookie_game::MAX_CURSORS as u64 * 8 {
            let index = (address - cursor_table) / 8;
            let entry = cursor_table + index * 8;
            let value = self.machine.memory.read_u64(entry).ok()?;
            let mut info = format!(
                "{}*[{index}]\n{}\n{} 0x{value:016X}\n{} 0x{entry:016X}",
                text("schema.cursor"),
                text("schema.pointer"),
                text("schema.value"),
                text("schema.address")
            );
            self.append_pointer_preview(&mut info, address);
            return Some(info);
        }
        for (a, name) in [
            (cookie_game::FUNCTION_TABLE, text("functions.add_cookie")),
            (cookie_game::FUNCTION_TABLE + 8, text("functions.win")),
        ] {
            if address >= a && address < a + 8 {
                let value = self.machine.memory.read_u64(a).ok()?;
                let mut info = format!(
                    "{name}\n{}\n{} 0x{value:016X}",
                    text("schema.function_pointer"),
                    text("schema.value")
                );
                self.append_pointer_preview(&mut info, address);
                return Some(info);
            }
        }
        for (start, end, name) in [
            (
                cookie_game::FN_ADD_COOKIE,
                cookie_game::FN_ADD_COOKIE + 16,
                text("functions.add_cookie"),
            ),
            (
                cookie_game::FN_WIN,
                cookie_game::FN_WIN + 8,
                text("functions.win"),
            ),
        ] {
            if address >= start && address < end {
                return Some(format!(
                    "{name}\n{}\n{} +0x{:X}\n{} 0x{address:016X}",
                    text("schema.bytecode_function"),
                    text("schema.offset"),
                    address - start,
                    text("schema.address")
                ));
            }
        }
        for a in self.machine.allocations() {
            if address >= a.base && address < a.base + a.size {
                let offset = address - a.base;
                let (name, ty) = match a.semantic_type.as_str() {
                    "struct Cookies" => (text("schema.cookies"), text("schema.u64")),
                    "struct User" if offset < 8 => (text("schema.id"), text("schema.u64")),
                    "struct User" => (text("schema.username"), text("schema.char_pointer")),
                    "struct Cursor" if offset < 8 => (text("schema.id"), text("schema.u64")),
                    "struct Cursor" if offset < 16 => (text("schema.name"), text("schema.char8")),
                    "struct Cursor" if offset < 24 => {
                        (text("schema.description"), text("schema.char_pointer"))
                    }
                    "struct Cursor" => (text("schema.on_clicked"), text("schema.function_pointer")),
                    _ => (a.semantic_type.as_str(), text("schema.bytes")),
                };
                let field_base = a.base + (offset / 8) * 8;
                let value = match ty {
                    ty if ty == text("schema.u64") => self
                        .machine
                        .memory
                        .read_u64(field_base)
                        .ok()
                        .map(|value| format!("{value} (0x{value:016X})")),
                    ty if ty == text("schema.char8") => {
                        self.machine.read_memory(field_base, 8).ok().map(|bytes| {
                            let end = bytes
                                .iter()
                                .position(|byte| *byte == 0)
                                .unwrap_or(bytes.len());
                            format!("{:?}", String::from_utf8_lossy(&bytes[..end]))
                        })
                    }
                    ty if ty == text("schema.char_pointer")
                        || ty == text("schema.function_pointer") =>
                    {
                        self.machine.memory.read_u64(field_base).ok().map(|value| {
                            let symbol = match value {
                                cookie_game::FN_ADD_COOKIE => text("functions.add_cookie"),
                                cookie_game::FN_WIN => text("functions.win"),
                                _ => "",
                            };
                            if symbol.is_empty() {
                                format!("0x{value:016X}")
                            } else {
                                format!("0x{value:016X} ({symbol})")
                            }
                        })
                    }
                    _ => self.machine.read_memory(field_base, 8).ok().map(|bytes| {
                        bytes
                            .iter()
                            .map(|byte| {
                                if byte.is_ascii_graphic() {
                                    *byte as char
                                } else {
                                    '.'
                                }
                            })
                            .collect()
                    }),
                }
                .unwrap_or_else(|| text("schema.unreadable").into());
                let object = match a.semantic_type.as_str() {
                    "struct Cookies" => text("schema.object.cookies"),
                    "struct User" => text("schema.object.user"),
                    "struct Cursor" => text("schema.object.cursor"),
                    "char username[32]" => text("schema.object.username"),
                    "char Cursor.description[64]" => text("schema.object.description"),
                    _ => a.semantic_type.as_str(),
                };
                let mut info = format!(
                    "{name}\n{ty}\n{} {value}\n{object} +0x{:X}\n{} 0x{address:016X}",
                    text("schema.value"),
                    offset,
                    text("schema.address")
                );
                self.append_pointer_preview(&mut info, address);
                return Some(info);
            }
        }
        None
    }
    fn field_color(&self, address: Address) -> Option<Color32> {
        if (cookie_game::FN_ADD_COOKIE..cookie_game::FN_ADD_COOKIE + 16).contains(&address) {
            return Some(Color32::LIGHT_BLUE);
        }
        if (cookie_game::FN_WIN..cookie_game::FN_WIN + 8).contains(&address) {
            return Some(Color32::LIGHT_GREEN);
        }
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
    fn append_pointer_preview(&self, info: &mut String, address: Address) {
        let Some(base) = self.pointer_field_base(address) else {
            return;
        };
        let Ok(target) = self.machine.memory.read_u64(base) else {
            return;
        };
        info.push_str(&format!("\n{} 0x{target:016X}", text("schema.points_to")));
        if let Ok(bytes) = self.machine.read_memory(target, 16) {
            let hex = bytes
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<Vec<_>>()
                .join(" ");
            let ascii: String = bytes
                .iter()
                .map(|byte| {
                    if byte.is_ascii_graphic() {
                        *byte as char
                    } else {
                        '.'
                    }
                })
                .collect();
            info.push_str(&format!("\n{hex}\n{ascii}"));
        } else {
            info.push('\n');
            info.push_str(text("memory.unmapped"));
        }
    }
    fn pointer_field_base(&self, address: Address) -> Option<Address> {
        let g = Architecture::GLOBALS;
        for offset in [0, 8, 16, 40] {
            let base = g + offset;
            if address >= base && address < base + 8 {
                return Some(base);
            }
        }
        let cursor_table = g + globals::CURSOR_TABLE;
        if address >= cursor_table && address < cursor_table + cookie_game::MAX_CURSORS as u64 * 8 {
            return Some(cursor_table + ((address - cursor_table) / 8) * 8);
        }
        if (cookie_game::FUNCTION_TABLE..cookie_game::FUNCTION_TABLE + 16).contains(&address) {
            return Some(
                cookie_game::FUNCTION_TABLE + ((address - cookie_game::FUNCTION_TABLE) / 8) * 8,
            );
        }
        for a in self.machine.allocations() {
            if a.semantic_type == "struct User" && address >= a.base + 8 && address < a.base + 16 {
                return Some(a.base + 8);
            }
            if a.semantic_type == "struct Cursor" {
                if address >= a.base + 16 && address < a.base + 24 {
                    return Some(a.base + 16);
                }
                if address >= a.base + 24 && address < a.base + 32 {
                    return Some(a.base + 24);
                }
            }
        }
        None
    }
    fn is_pointer_field(&self, address: Address) -> bool {
        self.pointer_field_base(address).is_some()
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
                        (text("memory.section.text"), Architecture::CODE),
                        (text("memory.section.bss"), Architecture::GLOBALS),
                        (text("memory.section.heap"), Architecture::HEAP),
                        (
                            text("memory.section.functions"),
                            cookie_game::FUNCTION_TABLE,
                        ),
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
                    ui.horizontal_wrapped(|ui| {
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
                                .map_err(|_| text("error.hex_byte").into())
                            };
                            match parsed {
                                Ok(bytes) => {
                                    let result = self.machine.write_memory(address, &bytes);
                                    self.report(result)
                                }
                                Err(e) => self.status = e,
                            }
                        }
                        if self.pointer_field_base(address).is_some()
                            && ui.button(text("memory.follow")).clicked()
                            && let Some(target) = self.pointer_target(address)
                        {
                            self.jump_to(target);
                        }
                    });
                }
                if !self.status.is_empty() {
                    ui.separator();
                    ui.label(&self.status);
                }
                egui::ScrollArea::both().show(ui, |ui| {
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
                                                rt = rt.color(Color32::RED).underline();
                                                if let Some(color) = self.field_color(a) {
                                                    rt = rt.background_color(
                                                        color.gamma_multiply(0.2),
                                                    );
                                                }
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
                                            if i == 8 {
                                                ui.separator();
                                            }
                                            let a = base + i as u64;
                                            let mut rt =
                                                RichText::new(format!("{v:02X}")).monospace();
                                            if self.is_pointer_field(a) {
                                                rt = rt.color(Color32::RED).underline();
                                                if let Some(color) = self.field_color(a) {
                                                    rt = rt.background_color(
                                                        color.gamma_multiply(0.2),
                                                    );
                                                }
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
            });
        self.memory_open = open;
    }
    fn game(&mut self, ui: &mut egui::Ui, view: &GameView) {
        if view.won {
            ui.vertical_centered(|ui| {
                ui.heading(text("game.win.title"));
                if ui.button(text("memory.open")).clicked() {
                    self.memory_open = true;
                }
            });
            return;
        }
        ui.horizontal_wrapped(|ui| {
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
        ui.horizontal_wrapped(|ui| {
            let response = ui.label(format!("{} {}", text("game.user"), view.username));
            if self.pointer_mode {
                let mut inspect = false;
                response.on_hover_ui(|ui| {
                    ui.label(text("schema.username"));
                    inspect = ui
                        .link(RichText::new(format!("0x{:016X}", view.username_ptr)).monospace())
                        .clicked();
                });
                if inspect {
                    self.jump_to(view.username_ptr);
                }
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
                let mut inspect = false;
                cookie.clone().on_hover_ui(|ui| {
                    ui.label(text("schema.cookies_value"));
                    inspect = ui
                        .link(RichText::new(format!("0x{:016X}", view.cookies_ptr)).monospace())
                        .clicked();
                });
                if inspect {
                    self.jump_to(view.cookies_ptr);
                }
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
        ui.horizontal_wrapped(|ui| {
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
                    view.cookies >= FLAG_PRICE && !view.flag_owned,
                    egui::Button::new(format!("{} — {FLAG_PRICE}", text("game.flag.buy"))),
                )
                .clicked()
            {
                let r = self.machine.buy_flag();
                self.report(r);
            }
            if view.flag_owned {
                ui.label(text("game.flag.owned"));
            }
        });
        ui.separator();
        ui.heading(text("game.cursors"));
        for (i, c) in view.cursors.iter().enumerate() {
            ui.horizontal_wrapped(|ui| {
                let response = ui.label(format!("{} #{}", c.name, c.id));
                if self.pointer_mode {
                    let mut inspect = false;
                    response.on_hover_ui(|ui| {
                        ui.label(text("schema.cursor"));
                        inspect = ui
                            .link(RichText::new(format!("0x{:016X}", c.address)).monospace())
                            .clicked();
                    });
                    if inspect {
                        self.jump_to(c.address);
                    }
                    if ui
                        .link(RichText::new(format!("0x{:016X}", c.address)).monospace())
                        .clicked()
                    {
                        self.jump_to(c.address);
                    }
                }
                ui.label(&c.description);
                if ui.button(text("game.cursor.click")).clicked() {
                    self.invoke_cursor(i);
                }
                if ui.button(text("game.cursor.edit")).clicked() {
                    self.cursor_editor = Some(i);
                    self.cursor_name = c.name.clone();
                    self.cursor_description = c.description.clone();
                    self.jump_to(c.address);
                }
            });
        }
        if !self.status.is_empty() && !self.memory_open {
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
        assert_eq!(view.username, text("default.username"));
        assert!(app.status.contains("segmentation fault"));
    }
    #[test]
    fn invalid_function_pointer_restores_last_valid_state() {
        let mut app = CookieApp::headless(Machine::new(1).unwrap());
        for _ in 0..10 {
            app.machine.click().unwrap();
        }
        app.machine.buy_cursor().unwrap();
        app.safe_view().unwrap();
        let cursor = app.machine.view().unwrap().cursors[0].address;
        app.machine
            .write_memory(
                cursor + cookie_game::cursor_field::ON_CLICKED,
                &u64::MAX.to_le_bytes(),
            )
            .unwrap();
        app.invoke_cursor(0);
        assert_eq!(
            app.machine.view().unwrap().cursors[0].on_clicked,
            cookie_game::FN_ADD_COOKIE
        );
        assert!(app.status.contains("segmentation fault"));
    }
    #[test]
    fn invalid_cursor_data_pointer_restores_last_valid_state() {
        let mut app = CookieApp::headless(Machine::new(1).unwrap());
        for _ in 0..10 {
            app.machine.click().unwrap();
        }
        app.machine.buy_cursor().unwrap();
        let original = app.safe_view().unwrap().cursors[0].clone();
        let mut overflow = b"12345678".to_vec();
        overflow.extend(u64::MAX.to_le_bytes());
        app.machine.rename_cursor(0, &overflow).unwrap();
        let restored = app.safe_view().unwrap().cursors[0].clone();
        assert_eq!(restored.description_ptr, original.description_ptr);
        assert!(app.status.contains("segmentation fault"));
    }
}
