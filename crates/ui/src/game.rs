use crate::{
    CookieApp,
    theme::{compact, number},
};
use cookie_game::{
    ACHIEVEMENTS, GENERATORS, GameView, RECIPES, RushState, TICKS_PER_SECOND, UPGRADES,
};
use eframe::egui::{self, Align, Button, RichText, ScrollArea, Stroke, Vec2};
use vm_core::ExecutionBackend;
impl CookieApp {
    pub(crate) fn game_ui(&mut self, ui: &mut egui::Ui) {
        let view = match self.machine.view() {
            Ok(v) => v,
            Err(e) => {
                self.machine.set_paused(true);
                ui.heading(self.text("game.fault.title"));
                ui.colored_label(self.theme().danger, e.to_string());
                ui.label(self.text("game.fault.body"));
                return;
            }
        };
        ScrollArea::vertical()
            .id_salt("game_scroll")
            .show(ui, |ui| {
                egui::Frame::NONE.inner_margin(16.0).show(ui, |ui| {
                    if ui.available_width() >= 760.0 {
                        let width = ui.available_width();
                        let left = (width - 24.0) * 0.43;
                        ui.horizontal_top(|ui| {
                            ui.allocate_ui_with_layout(
                                Vec2::new(left, 0.0),
                                egui::Layout::top_down(Align::Min),
                                |ui| self.bakery_ui(ui, &view),
                            );
                            ui.add_space(16.0);
                            ui.allocate_ui_with_layout(
                                Vec2::new(width - left - 24.0, 0.0),
                                egui::Layout::top_down(Align::Min),
                                |ui| self.shop_ui(ui, &view),
                            );
                        });
                    } else {
                        self.bakery_ui(ui, &view);
                        ui.add_space(24.0);
                        self.shop_ui(ui, &view);
                    }
                });
            });
    }
    fn bakery_ui(&mut self, ui: &mut egui::Ui, v: &GameView) {
        let theme = self.theme();
        let text = self.resources.text.clone();
        let cookie_texture = self.resources.cookie.clone();
        ui.vertical_centered(|ui| {
            let balance = number(v.cookies);
            let font_size = (ui.available_width() / (balance.len() as f32 * 0.6)).clamp(24.0, 80.0);
            ui.label(RichText::new(balance).size(font_size).strong());
            ui.label(RichText::new(text.get("game.cookies.available")).color(theme.muted));
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    text.format("game.cookies.lifetime", &[("count", number(v.lifetime))]),
                )
                .size(14.0)
                .color(theme.muted),
            );
            ui.add_space(8.0);
            ui.label(text.format(
                "game.cookies.rates",
                &[
                    ("click", compact(v.click_power)),
                    ("second", compact(v.cps)),
                ],
            ));
            ui.add_space(16.0);
            let size = ui.available_width().min(240.0);
            let (_, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click());
            let response = response.on_hover_text(text.get("game.cookie.tooltip"));
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    true,
                    text.get("game.cookie.action"),
                )
            });
            let center = response.rect.center();
            let radius = size * 0.43;
            let p = ui.painter();
            let fill = if response.is_pointer_button_down_on() {
                theme.cookie_edge
            } else {
                theme.cookie
            };
            if let Some(texture) = &cookie_texture {
                p.image(
                    texture.id(),
                    response.rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    if response.is_pointer_button_down_on() {
                        egui::Color32::from_gray(210)
                    } else {
                        egui::Color32::WHITE
                    },
                );
            } else {
                p.circle_filled(center, radius, fill);
                p.circle_stroke(
                    center,
                    radius,
                    Stroke::new(
                        if response.has_focus() { 3.0 } else { 2.0 },
                        if response.has_focus() {
                            theme.text
                        } else {
                            theme.cookie_edge
                        },
                    ),
                );
                for (x, y, r) in [
                    (-0.36, -0.44, 0.07),
                    (0.17, -0.56, 0.06),
                    (0.48, -0.14, 0.09),
                    (-0.55, 0.13, 0.08),
                    (-0.13, 0.00, 0.08),
                    (0.09, 0.53, 0.09),
                    (-0.32, 0.53, 0.05),
                    (0.44, 0.37, 0.06),
                ] {
                    p.circle_filled(center + egui::vec2(x, y) * radius, radius * r, theme.chip);
                }
            }
            if response.clicked() {
                let r = self.machine.click();
                self.report(r);
            }
            if v.gift_available {
                ui.label(RichText::new(text.get("game.gift.title")).strong());
                ui.horizontal_wrapped(|ui| {
                    if ui.button(text.get("game.gift.boost")).clicked() {
                        let r = self.machine.claim_gift();
                        self.report(r);
                    }
                    if ui
                        .button(text.format(
                            "game.gift.shower",
                            &[("count", compact(v.activities.shower_reward))],
                        ))
                        .clicked()
                    {
                        let r = self.machine.claim_cookie_shower();
                        self.report(r);
                    }
                });
            } else if v.boost_remaining > 0 {
                ui.label(
                    RichText::new(text.format(
                        "game.gift.active",
                        &[(
                            "seconds",
                            v.boost_remaining.div_ceil(TICKS_PER_SECOND).to_string(),
                        )],
                    ))
                    .color(theme.accent),
                );
            }
            ui.add_space(16.0);
        });
        self.rush_ui(ui, v);
        self.enemy_ui(ui, v);
        ui.add_space(16.0);
        ui.collapsing(&v.name, |ui| {
            ui.label(text.get("game.bakery_name"));
            let edit = ui.text_edit_singleline(&mut self.name_draft);
            if self.debug_enabled() {
                edit.on_hover_text(text.get("game.bakery_name.raw_hint"));
            }
            if ui.button(text.get("game.bakery_name.rename")).clicked() {
                match name_bytes(&self.name_draft, self.debug_enabled()) {
                    Ok(bytes) => {
                        let result = self.machine.rename(&bytes);
                        self.report(result);
                    }
                    Err(error) => self.report::<(), _>(Err(error)),
                }
            }
        });
        ui.collapsing(text.get("game.statistics"), |ui| {
            egui::Grid::new("statistics").num_columns(2).show(ui, |ui| {
                for (label, value) in [
                    (text.get("game.statistics.clicks"), number(v.clicks)),
                    (text.get("game.statistics.spent"), number(v.spent)),
                    (
                        text.get("game.statistics.time"),
                        text.format(
                            "game.statistics.duration",
                            &[
                                (
                                    "minutes",
                                    (v.play_ticks / TICKS_PER_SECOND / 60).to_string(),
                                ),
                                (
                                    "seconds",
                                    (v.play_ticks / TICKS_PER_SECOND % 60).to_string(),
                                ),
                            ],
                        ),
                    ),
                    (text.get("game.statistics.prestige"), number(v.prestige)),
                    (
                        text.get("game.statistics.rushes"),
                        number(v.activities.rush_wins),
                    ),
                    (
                        text.get("game.statistics.visitors"),
                        number(v.encounter.defeated),
                    ),
                ] {
                    ui.label(label);
                    ui.label(value);
                    ui.end_row();
                }
            });
        });
        ui.collapsing(
            text.format(
                "game.achievements",
                &[
                    (
                        "owned",
                        v.achievements.iter().filter(|x| **x).count().to_string(),
                    ),
                    ("total", ACHIEVEMENTS.len().to_string()),
                ],
            ),
            |ui| {
                for (i, (_, threshold)) in ACHIEVEMENTS.iter().enumerate() {
                    ui.label(text.format(
                        "game.achievement.row",
                        &[
                            ("mark", if v.achievements[i] { "✓" } else { "○" }.into()),
                            ("name", text.get(&format!("achievement.{i}.name")).into()),
                            ("count", compact(*threshold)),
                        ],
                    ));
                }
            },
        );
        ui.collapsing(text.get("game.prestige"), |ui| {
            ui.label(text.format(
                "game.prestige.points",
                &[("count", v.prestige_gain.to_string())],
            ));
            ui.small(text.get("game.prestige.help"));
            if ui
                .add_enabled(
                    v.prestige_gain > 0,
                    Button::new(text.get("game.prestige.reset")),
                )
                .clicked()
            {
                self.confirm_prestige = true;
            }
        });
    }
    fn shop_ui(&mut self, ui: &mut egui::Ui, v: &GameView) {
        let text = self.resources.text.clone();
        ui.scope(|ui| {
            ui.spacing_mut().button_padding.x = 8.0;
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.shop_tab, 0, text.get("game.tab.generators"));
                ui.selectable_value(&mut self.shop_tab, 1, text.get("game.tab.upgrades"));
                ui.selectable_value(&mut self.shop_tab, 2, text.get("game.tab.recipes"));
            });
        });
        ui.add_space(16.0);
        if self.shop_tab == 0 {
            for (i, _spec) in GENERATORS.iter().enumerate() {
                let g = &v.generators[i];
                let muted = self.theme().muted;
                activity_row(
                    ui,
                    |ui| {
                        let name = text.get(&format!("generator.{i}.name"));
                        ui.label(RichText::new(name).strong());
                        ui.label(
                            RichText::new(text.format(
                                "game.generator.detail",
                                &[("owned", compact(g.owned)), ("rate", compact(g.per_unit))],
                            ))
                            .small()
                            .color(muted),
                        )
                        .on_hover_text(text.format(
                            "game.generator.tooltip",
                            &[("rate", number(g.production)), ("name", name.to_owned())],
                        ));
                    },
                    |ui| {
                        if ui
                            .add_enabled(
                                v.cookies >= g.price,
                                Button::new(
                                    text.format("game.buy", &[("price", compact(g.price))]),
                                ),
                            )
                            .clicked()
                        {
                            let r = self.machine.buy_generator(i);
                            self.report(r);
                        }
                    },
                );
                ui.add_space(16.0);
            }
        } else if self.shop_tab == 1 {
            let muted = self.theme().muted;
            for (i, spec) in UPGRADES.iter().enumerate() {
                let unlocked = v.lifetime >= spec.unlock;
                activity_row(
                    ui,
                    |ui| {
                        ui.label(RichText::new(text.get(&format!("upgrade.{i}.name"))).strong());
                        ui.small(text.get(&format!("upgrade.{i}.description")));
                        if !unlocked {
                            ui.label(
                                RichText::new(text.format(
                                    "game.upgrade.unlock",
                                    &[("count", compact(spec.unlock))],
                                ))
                                .small()
                                .color(muted),
                            );
                        }
                    },
                    |ui| {
                        if v.upgrades[i] {
                            ui.label(text.get("game.upgrade.owned"));
                        } else if ui
                            .add_enabled(
                                unlocked && v.cookies >= spec.price,
                                Button::new(
                                    text.format("game.buy", &[("price", compact(spec.price))]),
                                ),
                            )
                            .on_hover_text(
                                text.format("game.upgrade.cost", &[("count", number(spec.price))]),
                            )
                            .clicked()
                        {
                            let r = self.machine.buy_upgrade(i);
                            self.report(r);
                        }
                    },
                );
                ui.add_space(16.0);
            }
        } else {
            self.recipes_ui(ui, v);
        }
    }
    fn rush_ui(&mut self, ui: &mut egui::Ui, v: &GameView) {
        let a = &v.activities;
        let text = self.resources.text.clone();
        if matches!(a.rush_state, RushState::Idle | RushState::Missed) {
            return;
        }
        activity_row(
            ui,
            |ui| {
                ui.label(RichText::new(text.get("game.rush.title")).strong());
                if a.rush_state == RushState::Offered {
                    ui.small(text.format(
                        "game.rush.offer",
                        &[
                            ("clicks", number(a.rush_target)),
                            ("reward", compact(a.rush_reward)),
                        ],
                    ));
                } else if a.rush_state == RushState::Running {
                    ui.small(text.format(
                        "game.rush.time",
                        &[(
                            "seconds",
                            a.rush_remaining.div_ceil(TICKS_PER_SECOND).to_string(),
                        )],
                    ));
                }
            },
            |ui| match a.rush_state {
                RushState::Offered => {
                    if ui
                        .button(text.get("game.rush.start"))
                        .on_hover_text(text.format(
                            "game.rush.deadline",
                            &[(
                                "seconds",
                                a.rush_remaining.div_ceil(TICKS_PER_SECOND).to_string(),
                            )],
                        ))
                        .clicked()
                    {
                        let r = self.machine.start_rush();
                        self.report(r);
                    }
                }
                RushState::Ready
                    if ui
                        .button(
                            text.format("game.rush.collect", &[("reward", compact(a.rush_reward))]),
                        )
                        .clicked() =>
                {
                    let r = self.machine.claim_rush();
                    self.report(r);
                    if !self.error {
                        self.status =
                            text.format("game.rush.complete", &[("reward", number(a.rush_reward))]);
                    }
                }
                _ => {}
            },
        );
        if a.rush_state == RushState::Running {
            ui.add(
                egui::ProgressBar::new(progress(a.rush_progress, a.rush_target)).text(text.format(
                    "game.rush.progress",
                    &[
                        ("current", compact(a.rush_progress)),
                        ("target", compact(a.rush_target)),
                    ],
                )),
            );
        }
        ui.add_space(16.0);
    }
    fn enemy_ui(&mut self, ui: &mut egui::Ui, v: &GameView) {
        let Some(name) = v.encounter.name else {
            return;
        };
        let theme = self.theme();
        let text = self.resources.text.clone();
        let visitor_texture = self.resources.visitor.clone();
        let display_name = match name {
            "Sugar ants" => text.get("visitor.sugar_ants"),
            "Crumb mouse" => text.get("visitor.crumb_mouse"),
            "Cookie gobbler" => text.get("visitor.cookie_gobbler"),
            _ => name,
        };
        activity_row(
            ui,
            |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(40.0), egui::Sense::hover());
                    let p = ui.painter();
                    if let Some(texture) = &visitor_texture {
                        p.image(
                            texture.id(),
                            rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    } else {
                        let c = rect.center();
                        p.circle_filled(c + egui::vec2(-12.0, -12.0), 7.0, theme.cookie_edge);
                        p.circle_filled(c + egui::vec2(12.0, -12.0), 7.0, theme.cookie_edge);
                        p.circle_filled(c, 17.0, theme.cookie_edge);
                        for x in [-6.0, 6.0] {
                            p.circle_filled(c + egui::vec2(x, -4.0), 5.0, theme.surface);
                            p.circle_filled(c + egui::vec2(x, -3.0), 2.5, theme.text);
                        }
                        p.circle_filled(c + egui::vec2(0.0, 12.0), 7.0, theme.cookie);
                    }
                    ui.vertical(|ui| {
                        ui.label(RichText::new(display_name).strong());
                        ui.small(text.format(
                            "game.visitor.detail",
                            &[
                                ("rate", compact(v.encounter.bite)),
                                ("count", compact(v.encounter.stolen)),
                            ],
                        ));
                    });
                });
            },
            |ui| {
                if ui
                    .button(text.format(
                        "game.visitor.shoo",
                        &[("health", number(v.encounter.health))],
                    ))
                    .on_hover_text(text.get("game.visitor.tooltip"))
                    .clicked()
                {
                    match self.machine.shoo_enemy() {
                        Ok(recovered) => {
                            self.error = false;
                            if v.encounter.health <= 1 {
                                self.status = text.format(
                                    "game.visitor.complete",
                                    &[("count", number(recovered))],
                                );
                            }
                        }
                        Err(e) => self.report::<(), _>(Err(e)),
                    }
                }
            },
        );
        ui.add_space(16.0);
    }
    fn recipes_ui(&mut self, ui: &mut egui::Ui, v: &GameView) {
        let text = self.resources.text.clone();
        for (i, recipe) in RECIPES.iter().enumerate() {
            ui.push_id(("recipe", i), |ui| {
                activity_row(
                    ui,
                    |ui| {
                        ui.label(RichText::new(text.get(&format!("recipe.{i}.name"))).strong());
                        ui.small(text.get(&format!("recipe.{i}.description")));
                    },
                    |ui| {
                        if v.activities.recipe == i {
                            ui.label(text.get("game.recipe.active"));
                        } else {
                            let unlocked = v.activities.recipes_unlocked[i];
                            if ui
                                .add_enabled(
                                    unlocked || v.cookies >= recipe.price,
                                    Button::new(if unlocked {
                                        text.get("game.recipe.use").into()
                                    } else {
                                        text.format(
                                            "game.recipe.unlock",
                                            &[("price", compact(recipe.price))],
                                        )
                                    }),
                                )
                                .clicked()
                            {
                                let r = self.machine.select_recipe(i);
                                self.report(r);
                            }
                        }
                    },
                );
                ui.add_space(24.0);
            });
        }
    }
}
pub(crate) fn name_bytes(value: &str, allow_raw: bool) -> Result<Vec<u8>, String> {
    let Some(raw) = value.strip_prefix("hex:").filter(|_| allow_raw) else {
        return Ok(value.as_bytes().to_vec());
    };
    let digits = raw
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect::<String>();
    if digits.is_empty() || !digits.len().is_multiple_of(2) {
        return Err("hex name payload must contain complete byte pairs".into());
    }
    (0..digits.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&digits[index..index + 2], 16)
                .map_err(|_| "hex name payload contains a non-hex byte".to_string())
        })
        .collect()
}
fn activity_row(
    ui: &mut egui::Ui,
    content: impl FnOnce(&mut egui::Ui),
    action: impl FnOnce(&mut egui::Ui),
) {
    let width = ui.available_width();
    let content_width = (width - (width * 0.44).min(164.0) - ui.spacing().item_spacing.x).max(0.0);
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(content_width, 0.0),
            egui::Layout::top_down(Align::Min),
            |ui| {
                ui.set_max_width(content_width);
                content(ui);
            },
        );
        ui.with_layout(egui::Layout::right_to_left(Align::Center), action);
    });
}
fn progress(current: u64, target: u64) -> f32 {
    if target == 0 {
        1.0
    } else {
        (current as f64 / target as f64).clamp(0.0, 1.0) as f32
    }
}
