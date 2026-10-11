//! Native application. Debugger capability is fixed at construction.
#![forbid(unsafe_code)]
mod game;
mod resources;
mod theme;
mod workspace;
use crate::resources::ResourcePack;
use cookie_game::{Machine, ShutdownOutcome};
use eframe::egui::{self, Button, RichText};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use vm_core::ExecutionBackend;
use workspace::DebugWorkspace;

pub fn window_title() -> String {
    resources::window_title()
}

pub struct CookieApp {
    pub(crate) machine: Machine,
    debugger: Option<DebugWorkspace>,
    dark: bool,
    pub(crate) shop_tab: u8,
    pub(crate) name_draft: String,
    status: String,
    error: bool,
    last_frame: Instant,
    accumulator: Duration,
    pending_load: Option<(PathBuf, Machine)>,
    confirm_new: bool,
    pub(crate) confirm_prestige: bool,
    confirm_exit: bool,
    shell_open: bool,
    shell_input: String,
    shell_lines: Vec<String>,
    resources: ResourcePack,
}
impl CookieApp {
    pub fn new(cc: &eframe::CreationContext<'_>, machine: Machine, debug_enabled: bool) -> Self {
        let mut app = Self::headless(machine, debug_enabled);
        app.resources = ResourcePack::load(&cc.egui_ctx);
        app.status = app.text("status.fresh").to_owned();
        app.theme().apply(&cc.egui_ctx, app.dark);
        app
    }
    pub fn headless(machine: Machine, debug_enabled: bool) -> Self {
        let name_draft = machine.view().map(|v| v.name).unwrap_or_default();
        Self {
            machine,
            debugger: debug_enabled.then(DebugWorkspace::new),
            dark: false,
            shop_tab: 0,
            name_draft,
            status: "Fresh session · save explicitly to keep it".into(),
            error: false,
            last_frame: Instant::now(),
            accumulator: Duration::ZERO,
            pending_load: None,
            confirm_new: false,
            confirm_prestige: false,
            confirm_exit: false,
            shell_open: false,
            shell_input: String::new(),
            shell_lines: vec![],
            resources: ResourcePack::built_in(),
        }
    }
    pub fn debug_enabled(&self) -> bool {
        self.debugger.is_some()
    }
    pub(crate) fn theme(&self) -> theme::Theme {
        theme::Theme::new(self.dark)
    }
    pub(crate) fn text(&self, key: &str) -> &str {
        self.resources.text.get(key)
    }
    pub(crate) fn format_text(&self, key: &str, values: &[(&str, String)]) -> String {
        self.resources.text.format(key, values)
    }
    pub(crate) fn report<T, E: std::fmt::Display>(&mut self, result: Result<T, E>) {
        match result {
            Ok(_) => {
                self.error = false;
                self.status = self.text("status.done").to_owned();
            }
            Err(e) => {
                self.error = true;
                self.status = e.to_string();
            }
        }
    }
    fn save_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(self.text("save.filter"), &["chls"])
            .set_file_name(self.text("save.default_name"))
            .save_file()
        {
            let result = save_state::save(&path, &self.machine);
            self.report(result);
            if !self.error {
                self.status =
                    self.format_text("status.saved", &[("path", path.display().to_string())]);
            }
        }
        self.last_frame = Instant::now();
    }
    fn select_load(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(self.text("save.filter"), &["chls"])
            .pick_file()
        {
            match save_state::load(&path) {
                Ok(m) => self.pending_load = Some((path, m)),
                Err(e) => self.report::<(), _>(Err(e)),
            }
        }
        self.last_frame = Instant::now();
    }
    fn replace_machine(&mut self, machine: Machine) {
        self.machine = machine;
        self.name_draft = self.machine.view().map(|v| v.name).unwrap_or_default();
        if self.debugger.is_some() {
            self.debugger = Some(DebugWorkspace::new());
        }
        self.accumulator = Duration::ZERO;
        self.last_frame = Instant::now();
    }
    fn tick(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame);
        self.last_frame = now;
        if self.machine.is_paused()
            || self.pending_load.is_some()
            || self.confirm_new
            || self.confirm_prestige
            || self.confirm_exit
        {
            self.accumulator = Duration::ZERO;
            return;
        }
        // Fixed simulated ticks, bounded catch-up. Never compute time since a saved wall clock.
        self.accumulator += elapsed.min(Duration::from_millis(250));
        let step = Duration::from_millis(50);
        while self.accumulator >= step {
            self.accumulator -= step;
            if let Err(e) = self.machine.advance_tick() {
                self.machine.set_paused(true);
                self.report::<(), _>(Err(e));
                break;
            }
        }
    }
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let text = self.resources.text.clone();
        if ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::S,
            ))
        }) {
            self.save_as();
        }
        if ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::O,
            ))
        }) {
            self.select_load();
        }
        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.label(RichText::new(text.get("app.title")).strong());
                ui.menu_button(text.get("menu.file"), |ui| {
                    if ui.button(text.get("menu.file.save")).clicked() {
                        ui.close();
                        self.save_as();
                    }
                    if ui.button(text.get("menu.file.load")).clicked() {
                        ui.close();
                        self.select_load();
                    }
                    if ui.button(text.get("menu.file.new")).clicked() {
                        ui.close();
                        self.confirm_new = true;
                    }
                    ui.separator();
                    if ui.button(text.get("menu.file.quit")).clicked() {
                        ui.close();
                        self.confirm_exit = true;
                    }
                });
                ui.menu_button(text.get("menu.appearance"), |ui| {
                    let light = ui
                        .selectable_value(&mut self.dark, false, text.get("menu.appearance.light"))
                        .changed();
                    let dark = ui
                        .selectable_value(&mut self.dark, true, text.get("menu.appearance.dark"))
                        .changed();
                    if light || dark {
                        self.theme().apply(&ctx, self.dark);
                    }
                    if ui.button(text.get("menu.appearance.reload")).clicked() {
                        self.resources = ResourcePack::load(&ctx);
                        self.status = self.format_text(
                            "status.resources",
                            &[("path", self.resources.source.clone())],
                        );
                        self.error = false;
                        ui.close();
                    }
                });
                if let Some(debug) = &mut self.debugger {
                    debug.menu(ui, &text);
                    ui.menu_button(text.get("menu.simulation"), |ui| {
                        let paused = self.machine.is_paused();
                        let action = if paused {
                            text.get("menu.simulation.resume")
                        } else {
                            text.get("menu.simulation.pause")
                        };
                        if ui.button(action).clicked() {
                            self.machine.set_paused(!paused);
                            self.accumulator = Duration::ZERO;
                            ui.close();
                        }
                        if ui
                            .add_enabled(paused, Button::new(text.get("menu.simulation.step")))
                            .clicked()
                        {
                            let r = self.machine.step();
                            self.report(r);
                        }
                    });
                }
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if self.error {
                    ui.colored_label(self.theme().danger, &self.status);
                } else {
                    ui.label(
                        RichText::new(&self.status)
                            .small()
                            .color(self.theme().muted),
                    );
                }
                if self.machine.is_paused() {
                    ui.label(text.get("status.paused"));
                }
            });
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(self.theme().canvas))
            .show(ui, |ui| {
                self.game_ui(ui);
                if let Some(mut debug) = self.debugger.take() {
                    debug.show(ui, self);
                    self.debugger = Some(debug);
                }
            });
        if self.pending_load.is_some() {
            let mut load = false;
            let mut cancel = false;
            egui::Modal::new(egui::Id::new("confirm_load")).show(&ctx, |ui| {
                ui.set_max_width(440.0);
                ui.heading(text.get("dialog.load.title"));
                ui.label(text.get("dialog.load.body"));
                if let Some((p, _)) = &self.pending_load {
                    ui.label(p.display().to_string());
                }
                ui.horizontal(|ui| {
                    load = ui.button(text.get("dialog.load.action")).clicked();
                    cancel = ui.button(text.get("common.cancel")).clicked();
                });
            });
            if load {
                let (p, m) = self.pending_load.take().unwrap();
                self.replace_machine(m);
                self.status =
                    self.format_text("status.loaded", &[("path", p.display().to_string())]);
                self.error = false;
            } else if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.pending_load = None;
            }
        }
        if self.confirm_new {
            let mut start = false;
            let mut cancel = false;
            egui::Modal::new(egui::Id::new("confirm_new")).show(&ctx, |ui| {
                ui.heading(text.get("dialog.new.title"));
                ui.label(text.get("dialog.new.body"));
                ui.horizontal(|ui| {
                    start = ui.button(text.get("dialog.new.action")).clicked();
                    cancel = ui.button(text.get("common.cancel")).clicked();
                });
            });
            if start {
                let seed = self.machine.next_random();
                match Machine::new(seed) {
                    Ok(m) => {
                        self.replace_machine(m);
                        self.status = self.text("status.fresh").to_owned();
                        self.error = false;
                    }
                    Err(e) => self.report::<(), _>(Err(e)),
                }
                self.confirm_new = false;
            } else if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.confirm_new = false;
            }
        }
        if self.confirm_prestige {
            let mut reset = false;
            let mut cancel = false;
            egui::Modal::new(egui::Id::new("confirm_prestige")).show(&ctx, |ui| {
                ui.heading(text.get("dialog.prestige.title"));
                ui.label(text.get("dialog.prestige.body"));
                ui.horizontal(|ui| {
                    reset = ui.button(text.get("dialog.prestige.action")).clicked();
                    cancel = ui.button(text.get("common.cancel")).clicked();
                });
            });
            if reset {
                let r = self.machine.prestige();
                self.report(r);
                self.confirm_prestige = false;
            } else if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.confirm_prestige = false;
            }
        }
        if self.confirm_exit {
            let mut exit = false;
            let mut cancel = false;
            egui::Modal::new(egui::Id::new("confirm_exit")).show(&ctx, |ui| {
                ui.heading(text.get("dialog.quit.title"));
                ui.label(text.get("dialog.quit.body"));
                ui.horizontal(|ui| {
                    exit = ui.button(text.get("dialog.quit.action")).clicked();
                    cancel = ui.button(text.get("common.cancel")).clicked();
                });
            });
            if exit {
                match self.machine.run_shutdown_callbacks() {
                    Ok(ShutdownOutcome::Close) => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close)
                    }
                    Ok(ShutdownOutcome::GuestShell) => {
                        self.shell_open = true;
                        self.shell_lines = vec![
                            self.text("shell.redirected").into(),
                            self.text("shell.ready").into(),
                        ];
                        self.error = false;
                        self.status = self.text("shell.status").into();
                    }
                    Err(e) => self.report::<(), _>(Err(e)),
                }
                self.confirm_exit = false;
            } else if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.confirm_exit = false;
            }
        }
        self.guest_shell_ui(&ctx);
        ctx.request_repaint_after(Duration::from_millis(50));
    }

    fn guest_shell_ui(&mut self, ctx: &egui::Context) {
        if !self.shell_open {
            return;
        }
        let mut open = true;
        let mut submit = false;
        let text = self.resources.text.clone();
        egui::Window::new(text.get("shell.title"))
            .id(egui::Id::new("guest_shell"))
            .open(&mut open)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(text.get("shell.subtitle"))
                        .small()
                        .color(self.theme().muted),
                );
                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for line in &self.shell_lines {
                            ui.label(RichText::new(line).monospace());
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("$ ").monospace());
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.shell_input)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY),
                    );
                    submit = (response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                        || ui.button(text.get("shell.run")).clicked();
                });
            });
        self.shell_open = open;
        if submit {
            self.run_guest_command();
        }
    }

    fn run_guest_command(&mut self) {
        let command = self
            .shell_input
            .trim()
            .chars()
            .take(128)
            .collect::<String>();
        self.shell_input.clear();
        if command.is_empty() {
            return;
        }
        self.shell_lines.push(format!("$ {command}"));
        match command.as_str() {
            "help" => self.shell_lines.push(self.text("shell.help").into()),
            "id" => self.shell_lines.push(self.text("shell.id").into()),
            "pwd" => self.shell_lines.push(self.text("shell.pwd").into()),
            "ls" => self.shell_lines.push(self.text("shell.ls").into()),
            "cat flag" => self.shell_lines.push(self.text("shell.flag").into()),
            "exit" => {
                self.shell_lines.push(self.text("shell.logout").into());
                self.shell_open = false;
            }
            _ => self
                .shell_lines
                .push(self.format_text("shell.unknown", &[("command", command)])),
        }
    }
}
impl eframe::App for CookieApp {
    fn logic(&mut self, _: &egui::Context, _: &mut eframe::Frame) {
        self.tick();
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.show(ui);
    }
    fn persist_egui_memory(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(crate) fn render(
        ctx: &egui::Context,
        app: &mut CookieApp,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 800.0),
                )),
                events,
                ..Default::default()
            },
            |ui| app.show(ui),
        )
    }
    #[test]
    fn cookie_picture_accepts_pointer_clicks() {
        for debug in [false, true] {
            let ctx = egui::Context::default();
            let mut app = CookieApp::headless(Machine::new(1234).unwrap(), debug);
            app.theme().apply(&ctx, false);
            render(&ctx, &mut app, vec![]).drop_without_applying_deltas();
            let output = render(&ctx, &mut app, vec![]);
            let center = output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::epaint::Shape::Circle(c) if c.radius > 50.0 => Some(c.center),
                    _ => None,
                })
                .expect("cookie must be visible");
            output.drop_without_applying_deltas();
            for pressed in [true, false] {
                render(
                    &ctx,
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(center),
                        egui::Event::PointerButton {
                            pos: center,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                )
                .drop_without_applying_deltas();
            }
            assert_eq!(
                app.machine.view().unwrap().cookies,
                1,
                "clicking the visible cookie must bake exactly once in debug={debug}"
            );
        }
    }
    pub(crate) fn texts(
        shape: &egui::epaint::Shape,
        out: &mut Vec<(String, egui::Color32, f32, egui::Pos2)>,
    ) {
        match shape {
            egui::epaint::Shape::Text(t) => {
                let section = &t.galley.job.sections[0];
                let color = t.override_text_color.unwrap_or(
                    if section.format.color == egui::Color32::PLACEHOLDER {
                        t.fallback_color
                    } else {
                        section.format.color
                    },
                );
                out.push((
                    t.galley.job.text.clone(),
                    color,
                    section.format.font_id.size,
                    t.pos,
                ));
            }
            egui::epaint::Shape::Vec(shapes) => {
                for s in shapes {
                    texts(s, out);
                }
            }
            _ => {}
        }
    }
    #[test]
    fn strong_labels_and_shop_tabs_use_readable_theme_text() {
        let ctx = egui::Context::default();
        let mut app = CookieApp::headless(Machine::new(1).unwrap(), false);
        for _ in 0..20 {
            app.machine.click().unwrap();
        }
        app.machine.buy_generator(0).unwrap();
        for dark in [false, true, false] {
            app.dark = dark;
            let theme = app.theme();
            theme.apply(&ctx, dark);
            let output = render(&ctx, &mut app, vec![]);
            let mut painted = vec![];
            for s in &output.shapes {
                texts(&s.shape, &mut painted);
            }
            for name in ["Cookie Heap Lab", "Cursor", "Generators"] {
                let (_, color, _, _) = painted
                    .iter()
                    .find(|(text, _, _, _)| text == name)
                    .unwrap_or_else(|| panic!("{name} must be painted"));
                assert_eq!(
                    *color, theme.text,
                    "{name} must use readable text in dark={dark}"
                );
            }
            let (_, balance_color, size, _) = painted.iter().find(|(t, _, _, _)| t == "5").unwrap();
            let (_, lifetime_color, lifetime_size, _) = painted
                .iter()
                .find(|(t, _, _, _)| t == "20 total cookies baked")
                .unwrap();
            assert!(*size >= 64.0, "available cookies should be prominent");
            assert!(
                *size > *lifetime_size,
                "available cookies must be bigger than lifetime cookies"
            );
            assert_eq!(*balance_color, theme.text);
            assert_eq!(*lifetime_color, theme.muted);
            assert!(painted.iter().any(|(t, _, _, _)| t == "cookies available"));
            output.drop_without_applying_deltas();
        }
    }
    pub(crate) fn click_label(ctx: &egui::Context, app: &mut CookieApp, label: &str) {
        render(ctx, app, vec![]).drop_without_applying_deltas();
        let output = render(ctx, app, vec![]);
        let mut painted = vec![];
        for s in &output.shapes {
            texts(&s.shape, &mut painted);
        }
        let (_, _, _, pos) = painted
            .iter()
            .find(|(text, _, _, _)| text == label)
            .unwrap_or_else(|| panic!("{label} must be visible"));
        let pos = *pos + egui::vec2(4.0, 4.0);
        output.drop_without_applying_deltas();
        for pressed in [true, false] {
            render(
                ctx,
                app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            )
            .drop_without_applying_deltas();
        }
    }
    #[test]
    fn activity_controls_play_a_rush_and_unlock_a_recipe_without_orders() {
        for debug in [false, true] {
            for dark in [false, true] {
                let ctx = egui::Context::default();
                let mut app = CookieApp::headless(Machine::new(1).unwrap(), debug);
                app.dark = dark;
                app.theme().apply(&ctx, dark);
                let a = app.machine.activities_address().unwrap();
                app.machine
                    .write_memory(
                        a + cookie_game::activity_field::RUSH_STATE,
                        &4u64.to_le_bytes(),
                    )
                    .unwrap();
                app.machine
                    .write_memory(
                        a + cookie_game::activity_field::RUSH_DEADLINE,
                        &600u64.to_le_bytes(),
                    )
                    .unwrap();
                click_label(&ctx, &mut app, "Start rush");
                assert_eq!(
                    app.machine.activities_view().unwrap().rush_state,
                    cookie_game::RushState::Running
                );
                for _ in 0..12 {
                    app.machine.click().unwrap();
                }
                click_label(&ctx, &mut app, "Collect · +50");
                assert_eq!(app.machine.view().unwrap().cookies, 62);
                click_label(&ctx, &mut app, "Recipes");
                click_label(&ctx, &mut app, "Unlock · 40");
                assert_eq!(app.machine.view().unwrap().click_power, 3);
                assert_eq!(app.machine.view().unwrap().cookies, 22);
                let output = render(&ctx, &mut app, vec![]);
                let mut painted = vec![];
                for shape in &output.shapes {
                    texts(&shape.shape, &mut painted);
                }
                assert!(
                    painted
                        .iter()
                        .all(|(text, _, _, _)| !text.starts_with("Orders"))
                );
                output.drop_without_applying_deltas();
            }
        }
    }
    #[test]
    fn debug_name_payload_and_guest_shell_are_bounded_simulations() {
        assert_eq!(
            crate::game::name_bytes("hex: 80 01 40 00", true).unwrap(),
            [0x80, 0x01, 0x40, 0x00]
        );
        assert_eq!(
            crate::game::name_bytes("hex: 80", false).unwrap(),
            b"hex: 80"
        );
        assert!(crate::game::name_bytes("hex: 8", true).is_err());

        let mut app = CookieApp::headless(Machine::new(1).unwrap(), true);
        app.shell_open = true;
        app.shell_input = "cat flag".into();
        app.run_guest_command();
        assert_eq!(app.shell_lines.last().unwrap(), "CHL{vtable_call_hijacked}");
        app.shell_input = "uname -a".into();
        app.run_guest_command();
        assert_eq!(
            app.shell_lines.last().unwrap(),
            "uname -a: command not found"
        );
    }
}
