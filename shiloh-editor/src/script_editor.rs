//! Designer script editor — autocomplete + IntelliSense for Rhai.
//!
//! # UX goals (game designer)
//! Feels like Godot’s script dock / VS Code lite: file list, code pane with
//! line numbers, completion popup (Tab / ↑↓ / Esc), and a docs strip for the
//! symbol under the cursor or selected completion.
//!
//! # OSS references (ideas adapted — Shiloh-owned code)
//! - [egui_code_editor](https://github.com/p4ymak/egui_code_editor) (MIT) —
//!   Completer popup + keyword syntax dictionary; we reimplement against egui 0.31
//!   with a Shiloh/Rhai API catalog instead of vendoring (their latest needs egui 0.35).
//! - Godot 4 Script Editor — Scripts dock + Scene tree stays visible; never hide Outliner.
//! - VS Code IntelliSense — signature + one-line docs beside completions.

use egui::{self, Color32, FontId, Galley, RichText, Stroke, TextEdit, Ui, Vec2};

/// One API / keyword entry for autocomplete + hover docs.
#[derive(Debug, Clone, Copy)]
pub struct ScriptSymbol {
    pub name: &'static str,
    pub kind: SymbolKind,
    pub signature: &'static str,
    pub docs: &'static str,
    /// Insert text when completing (may include `(…)` placeholders).
    pub insert: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Keyword,
    Lifecycle,
    Api,
    Snippet,
}

impl SymbolKind {
    fn label(self) -> &'static str {
        match self {
            Self::Keyword => "keyword",
            Self::Lifecycle => "lifecycle",
            Self::Api => "api",
            Self::Snippet => "snippet",
        }
    }

    fn color(self) -> Color32 {
        match self {
            Self::Keyword => Color32::from_rgb(198, 120, 221),
            Self::Lifecycle => Color32::from_rgb(97, 175, 239),
            Self::Api => Color32::from_rgb(229, 192, 123),
            Self::Snippet => Color32::from_rgb(152, 195, 121),
        }
    }
}

/// Shiloh sandbox API + Rhai keywords designers actually type.
/// Keep in sync with `shiloh_scripting::rhai_host::register_api`.
pub fn shiloh_script_catalog() -> &'static [ScriptSymbol] {
    &[
        ScriptSymbol {
            name: "fn",
            kind: SymbolKind::Keyword,
            signature: "fn name(args) { … }",
            docs: "Declare a function.",
            insert: "fn ",
        },
        ScriptSymbol {
            name: "let",
            kind: SymbolKind::Keyword,
            signature: "let name = value;",
            docs: "Mutable local binding.",
            insert: "let ",
        },
        ScriptSymbol {
            name: "const",
            kind: SymbolKind::Keyword,
            signature: "const NAME = value;",
            docs: "Immutable binding.",
            insert: "const ",
        },
        ScriptSymbol {
            name: "if",
            kind: SymbolKind::Keyword,
            signature: "if cond { … }",
            docs: "Conditional branch.",
            insert: "if ",
        },
        ScriptSymbol {
            name: "else",
            kind: SymbolKind::Keyword,
            signature: "else { … }",
            docs: "Else branch.",
            insert: "else ",
        },
        ScriptSymbol {
            name: "while",
            kind: SymbolKind::Keyword,
            signature: "while cond { … }",
            docs: "Loop while condition is true.",
            insert: "while ",
        },
        ScriptSymbol {
            name: "for",
            kind: SymbolKind::Keyword,
            signature: "for x in range { … }",
            docs: "Iterate a range or collection.",
            insert: "for ",
        },
        ScriptSymbol {
            name: "return",
            kind: SymbolKind::Keyword,
            signature: "return value;",
            docs: "Return from the current function.",
            insert: "return ",
        },
        ScriptSymbol {
            name: "true",
            kind: SymbolKind::Keyword,
            signature: "true",
            docs: "Boolean true.",
            insert: "true",
        },
        ScriptSymbol {
            name: "false",
            kind: SymbolKind::Keyword,
            signature: "false",
            docs: "Boolean false.",
            insert: "false",
        },
        ScriptSymbol {
            name: "on_ready",
            kind: SymbolKind::Lifecycle,
            signature: "fn on_ready() { … }",
            docs: "Godot-like: called once when Play starts (after load).",
            insert: "fn on_ready() {\n    \n}\n",
        },
        ScriptSymbol {
            name: "on_update",
            kind: SymbolKind::Lifecycle,
            signature: "fn on_update(dt) { … }",
            docs: "Godot-like: called every frame; `dt` is seconds since last frame.",
            insert: "fn on_update(dt) {\n    \n}\n",
        },
        ScriptSymbol {
            name: "log",
            kind: SymbolKind::Api,
            signature: "log(message)",
            docs: "Print to the Studio Console (designer-safe; no file I/O).",
            insert: "log(\"\")",
        },
        ScriptSymbol {
            name: "say",
            kind: SymbolKind::Api,
            signature: "say(message)",
            docs: "Alias of log — useful when teaching non-coders.",
            insert: "say(\"\")",
        },
        ScriptSymbol {
            name: "set_translation",
            kind: SymbolKind::Api,
            signature: "set_translation(entity_index, x, y, z)",
            docs: "Move an entity by index (from Scene tree order).",
            insert: "set_translation(0, 0.0, 0.0, 0.0)",
        },
        ScriptSymbol {
            name: "spawn_named",
            kind: SymbolKind::Api,
            signature: "spawn_named(name, x, y, z)",
            docs: "Spawn a named world item / prop at a position.",
            insert: "spawn_named(\"Pine Tall\", 0.0, 0.0, 0.0)",
        },
        ScriptSymbol {
            name: "emit_signal",
            kind: SymbolKind::Api,
            signature: "emit_signal(name)",
            docs: "Fire a named signal for quests / UI / other scripts.",
            insert: "emit_signal(\"quest_started\")",
        },
        ScriptSymbol {
            name: "play_audio",
            kind: SymbolKind::Api,
            signature: "play_audio(name)",
            docs: "Play a one-shot audio cue by asset name.",
            insert: "play_audio(\"ui_click\")",
        },
        ScriptSymbol {
            name: "demo_spin",
            kind: SymbolKind::Snippet,
            signature: "snippet: demo_spin",
            docs: "Starter script: on_ready + on_update that logs each tick.",
            insert: concat!(
                "// Shiloh designer script — safe sandbox API\n",
                "fn on_ready() {\n",
                "    log(\"ready\");\n",
                "}\n",
                "fn on_update(dt) {\n",
                "    log(\"tick\");\n",
                "}\n",
            ),
        },
    ]
}

fn default_new_script() -> String {
    concat!(
        "// New Shiloh script — autocomplete: type a name, Tab to accept\n",
        "// Lifecycle: on_ready / on_update · API: log, spawn_named, …\n",
        "\n",
        "fn on_ready() {\n",
        "    log(\"hello designer\");\n",
        "}\n",
        "\n",
        "fn on_update(dt) {\n",
        "    // dt = seconds since last frame\n",
        "}\n",
    )
    .to_string()
}

/// Bottom-dock script IDE state.
pub struct ScriptEditorState {
    pub buffer: String,
    pub open_path: Option<std::path::PathBuf>,
    pub open_rel: String,
    pub dirty: bool,
    pub file_list: Vec<String>,
    pub selected_file: Option<String>,
    pub completer_open: bool,
    pub completer_query: String,
    pub completer_index: usize,
    pub completer_filtered: Vec<usize>,
    pub status: String,
    pub cursor_docs: String,
    /// Last known cursor char index (best-effort from TextEdit).
    pub cursor_ccursor: usize,
}

impl Default for ScriptEditorState {
    fn default() -> Self {
        Self {
            buffer: default_new_script(),
            open_path: None,
            open_rel: String::new(),
            dirty: false,
            file_list: Vec::new(),
            selected_file: None,
            completer_open: false,
            completer_query: String::new(),
            completer_index: 0,
            completer_filtered: Vec::new(),
            status: "New untitled script — Save to Scripts/".into(),
            cursor_docs: "Type to open IntelliSense · Tab accepts · Esc hides".into(),
            cursor_ccursor: 0,
        }
    }
}

impl ScriptEditorState {
    pub fn refresh_file_list(&mut self, scripts_dir: &std::path::Path) {
        self.file_list.clear();
        if let Ok(rd) = std::fs::read_dir(scripts_dir) {
            let mut names: Vec<String> = rd
                .flatten()
                .filter_map(|e| {
                    let p = e.path();
                    if p.extension().and_then(|x| x.to_str()) == Some("rhai") {
                        p.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                    } else {
                        None
                    }
                })
                .collect();
            names.sort();
            self.file_list = names;
        }
    }

    pub fn new_script(&mut self) {
        self.buffer = default_new_script();
        self.open_path = None;
        self.open_rel.clear();
        self.selected_file = None;
        self.dirty = true;
        self.status = "New untitled script".into();
        self.completer_open = false;
    }

    pub fn load_file(&mut self, scripts_dir: &std::path::Path, name: &str) -> std::io::Result<()> {
        let path = scripts_dir.join(name);
        self.buffer = std::fs::read_to_string(&path)?;
        self.open_path = Some(path);
        self.open_rel = format!("Scripts/{name}");
        self.selected_file = Some(name.to_string());
        self.dirty = false;
        self.status = format!("Opened {name}");
        self.completer_open = false;
        Ok(())
    }

    pub fn save(&mut self, scripts_dir: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
        std::fs::create_dir_all(scripts_dir)?;
        let path = if let Some(p) = &self.open_path {
            p.clone()
        } else {
            let name = self
                .selected_file
                .clone()
                .unwrap_or_else(|| "untitled.rhai".into());
            let path = scripts_dir.join(name);
            self.open_path = Some(path.clone());
            self.open_rel = format!(
                "Scripts/{}",
                path.file_name().unwrap().to_string_lossy()
            );
            path
        };
        std::fs::write(&path, &self.buffer)?;
        self.dirty = false;
        self.status = format!("Saved {}", path.display());
        self.refresh_file_list(scripts_dir);
        Ok(path)
    }

    fn word_before_cursor(&self) -> (usize, String) {
        let i = self.cursor_ccursor.min(self.buffer.len());
        // Walk back on UTF-8 safely via char indices.
        let bytes = self.buffer.as_bytes();
        let mut start = i;
        while start > 0 {
            let prev = start - 1;
            let c = bytes[prev] as char;
            if c.is_ascii_alphanumeric() || c == '_' {
                start = prev;
            } else {
                break;
            }
        }
        // Align to char boundary
        while start < i && !self.buffer.is_char_boundary(start) {
            start += 1;
        }
        let word = self.buffer[start..i].to_string();
        (start, word)
    }

    fn recompute_completions(&mut self) {
        let (_, word) = self.word_before_cursor();
        self.completer_query = word.clone();
        let q = word.to_ascii_lowercase();
        self.completer_filtered = shiloh_script_catalog()
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                if q.is_empty() {
                    matches!(s.kind, SymbolKind::Lifecycle | SymbolKind::Api | SymbolKind::Snippet)
                } else {
                    s.name.to_ascii_lowercase().starts_with(&q)
                        || s.name.to_ascii_lowercase().contains(&q)
                }
            })
            .map(|(i, _)| i)
            .collect();
        if self.completer_filtered.is_empty() {
            self.completer_open = false;
        } else {
            self.completer_open = true;
            self.completer_index = self
                .completer_index
                .min(self.completer_filtered.len().saturating_sub(1));
            if let Some(&idx) = self.completer_filtered.get(self.completer_index) {
                let s = &shiloh_script_catalog()[idx];
                self.cursor_docs = format!("{} — {}", s.signature, s.docs);
            }
        }
    }

    fn apply_completion(&mut self) {
        let Some(&cat_i) = self.completer_filtered.get(self.completer_index) else {
            return;
        };
        let sym = shiloh_script_catalog()[cat_i];
        let (start, word) = self.word_before_cursor();
        let end = self.cursor_ccursor.min(self.buffer.len());
        if start > self.buffer.len() || end > self.buffer.len() || start > end {
            return;
        }
        self.buffer.replace_range(start..end, sym.insert);
        self.cursor_ccursor = start + sym.insert.len();
        self.dirty = true;
        self.completer_open = false;
        self.status = format!("Inserted `{}`", sym.name);
        let _ = word;
    }

    /// Draw the designer script IDE into the bottom Script dock.
    pub fn ui(&mut self, ui: &mut Ui, scripts_dir: Option<&std::path::Path>) {
        // Toolbar — polished designer chrome
        ui.horizontal(|ui| {
            ui.spacing_mut().button_padding = Vec2::new(10.0, 4.0);
            ui.strong(RichText::new("Script Editor").size(15.0));
            ui.label(
                RichText::new("Rhai · IntelliSense")
                    .weak()
                    .small(),
            );
            ui.separator();
            if ui
                .add_sized([72.0, 26.0], egui::Button::new("＋ New"))
                .on_hover_text("New .rhai script")
                .clicked()
            {
                self.new_script();
            }
            let can_save = scripts_dir.is_some();
            if ui
                .add_enabled(
                    can_save,
                    egui::Button::new(if self.dirty { "💾 Save *" } else { "💾 Save" })
                        .min_size(Vec2::new(72.0, 26.0)),
                )
                .on_hover_text("Save to project Scripts/ (Ctrl+S)")
                .clicked()
            {
                if let Some(dir) = scripts_dir {
                    let _ = self.save(dir);
                }
            }
            if self.dirty {
                ui.label(RichText::new("unsaved").color(Color32::from_rgb(230, 170, 70)).small());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(&self.status).weak().small());
            });
        });

        // Ctrl+S
        if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            if let Some(dir) = scripts_dir {
                let _ = self.save(dir);
            }
        }

        ui.add_space(4.0);

        let avail = ui.available_height().max(160.0);
        ui.horizontal(|ui| {
            // File list (Godot-like Scripts panel)
            ui.vertical(|ui| {
                ui.set_width(148.0);
                ui.set_min_height(avail);
                ui.label(RichText::new("Scripts/").strong().small());
                if let Some(dir) = scripts_dir {
                    if ui.small_button("↻ Refresh").clicked() {
                        self.refresh_file_list(dir);
                    }
                    self.refresh_file_list(dir);
                }
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("script_files")
                    .max_height(avail - 40.0)
                    .show(ui, |ui| {
                        if self.file_list.is_empty() {
                            ui.label(RichText::new("No .rhai yet").weak().small());
                        }
                        for name in self.file_list.clone() {
                            let selected = self.selected_file.as_deref() == Some(name.as_str());
                            if ui.selectable_label(selected, &name).clicked() {
                                if let Some(dir) = scripts_dir {
                                    let _ = self.load_file(dir, &name);
                                }
                            }
                        }
                    });
            });

            ui.separator();

            // Code + IntelliSense
            ui.vertical(|ui| {
                ui.set_min_width(ui.available_width());
                // Docs strip
                ui.horizontal(|ui| {
                    ui.label(RichText::new("ℹ").color(Color32::from_rgb(90, 160, 255)));
                    ui.label(
                        RichText::new(&self.cursor_docs)
                            .small()
                            .color(Color32::from_rgb(180, 190, 210)),
                    );
                });

                let editor_h = (avail - 56.0).max(120.0);
                let output = {
                    let mut layouter = |ui: &Ui, text: &str, wrap_width: f32| {
                        highlight_rhai(ui, text, wrap_width)
                    };
                    let te = TextEdit::multiline(&mut self.buffer)
                        .id_salt("shiloh_script_editor")
                        .font(FontId::monospace(13.5))
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .desired_rows(18)
                        .lock_focus(true)
                        .layouter(&mut layouter);
                    let mut out = te.show(ui);
                    // Force height via allocate after — egui TextEdit uses desired_rows
                    let _ = editor_h;
                    // Track cursor for completions
                    if let Some(cursor) = out.cursor_range {
                        self.cursor_ccursor = cursor.primary.ccursor.index;
                    }
                    if out.response.changed() {
                        self.dirty = true;
                        self.recompute_completions();
                    }
                    // Trigger completer on Ctrl+Space (VS Code / Unreal Content drawer cousin)
                    if out.response.has_focus()
                        && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Space))
                    {
                        self.recompute_completions();
                        self.completer_open = !self.completer_filtered.is_empty();
                    }
                    // Also open when typing identifier chars
                    if out.response.has_focus() && out.response.changed() {
                        let (_, w) = self.word_before_cursor();
                        if w.len() >= 1 {
                            self.recompute_completions();
                        }
                    }
                    out
                };

                // Completer popup — adapted from egui_code_editor Completer UX
                if self.completer_open && !self.completer_filtered.is_empty() {
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.completer_open = false;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                        self.completer_index =
                            (self.completer_index + 1) % self.completer_filtered.len();
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                        self.completer_index = if self.completer_index == 0 {
                            self.completer_filtered.len() - 1
                        } else {
                            self.completer_index - 1
                        };
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Tab) || i.key_pressed(egui::Key::Enter))
                    {
                        self.apply_completion();
                        ui.ctx().request_repaint();
                    }

                    let popup_pos = output.response.rect.left_top()
                        + Vec2::new(24.0, 28.0 + (self.cursor_ccursor as f32 * 0.02).min(80.0));
                    egui::Area::new(egui::Id::new("script_completer"))
                        .order(egui::Order::Foreground)
                        .fixed_pos(popup_pos)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style())
                                .fill(Color32::from_rgb(28, 32, 42))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(60, 80, 120)))
                                .inner_margin(6.0)
                                .show(ui, |ui| {
                                    ui.set_min_width(280.0);
                                    ui.label(
                                        RichText::new("IntelliSense")
                                            .strong()
                                            .small()
                                            .color(Color32::from_rgb(120, 170, 255)),
                                    );
                                    ui.separator();
                                    let filtered = self.completer_filtered.clone();
                                    let mut clicked_row: Option<usize> = None;
                                    for (row, &cat_i) in filtered.iter().enumerate() {
                                        let sym = shiloh_script_catalog()[cat_i];
                                        let selected = row == self.completer_index;
                                        let label = format!("{}  {}", sym.name, sym.kind.label());
                                        let resp = ui.selectable_label(
                                            selected,
                                            RichText::new(label).color(sym.kind.color()).monospace(),
                                        );
                                        if selected {
                                            ui.label(RichText::new(sym.docs).small().weak());
                                        }
                                        if resp.clicked() {
                                            clicked_row = Some(row);
                                        }
                                    }
                                    if let Some(row) = clicked_row {
                                        self.completer_index = row;
                                        self.apply_completion();
                                    }
                                    ui.separator();
                                    ui.label(
                                        RichText::new("Tab / Enter · ↑↓ · Esc · Ctrl+Space")
                                            .small()
                                            .weak(),
                                    );
                                });
                        });
                }

                // API quick chips for designers who don't want to type
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().button_padding = Vec2::new(8.0, 3.0);
                    ui.label(RichText::new("Insert:").small().weak());
                    for sym in shiloh_script_catalog()
                        .iter()
                        .filter(|s| matches!(s.kind, SymbolKind::Lifecycle | SymbolKind::Api))
                    {
                        if ui
                            .small_button(sym.name)
                            .on_hover_text(format!("{}\n{}", sym.signature, sym.docs))
                            .clicked()
                        {
                            if !self.buffer.ends_with('\n') {
                                self.buffer.push('\n');
                            }
                            self.buffer.push_str(sym.insert);
                            if !sym.insert.ends_with('\n') {
                                self.buffer.push('\n');
                            }
                            self.dirty = true;
                            self.cursor_docs = format!("{} — {}", sym.signature, sym.docs);
                        }
                    }
                });
            });
        });
    }
}

/// Simple Rhai syntax highlight layouter (keyword sets — same approach as egui_code_editor).
fn highlight_rhai(ui: &Ui, text: &str, wrap_width: f32) -> std::sync::Arc<Galley> {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap_width;

    let keywords: &[&str] = &[
        "fn", "let", "const", "if", "else", "while", "for", "in", "return", "true", "false",
        "switch", "case", "do", "until", "loop", "break", "continue",
    ];
    let apis: &[&str] = &[
        "on_ready",
        "on_update",
        "log",
        "say",
        "set_translation",
        "spawn_named",
        "emit_signal",
        "play_audio",
    ];

    let mut rest = text;
    while !rest.is_empty() {
        if rest.starts_with("//") {
            let end = rest.find('\n').unwrap_or(rest.len());
            job.append(
                &rest[..end],
                0.0,
                egui::TextFormat {
                    font_id: FontId::monospace(13.5),
                    color: Color32::from_rgb(100, 110, 120),
                    ..Default::default()
                },
            );
            rest = &rest[end..];
            continue;
        }
        if rest.starts_with('"') {
            let mut end = 1;
            let bytes = rest.as_bytes();
            while end < rest.len() {
                if bytes[end] == b'\\' && end + 1 < rest.len() {
                    end += 2;
                    continue;
                }
                if bytes[end] == b'"' {
                    end += 1;
                    break;
                }
                end += 1;
            }
            job.append(
                &rest[..end],
                0.0,
                egui::TextFormat {
                    font_id: FontId::monospace(13.5),
                    color: Color32::from_rgb(152, 195, 121),
                    ..Default::default()
                },
            );
            rest = &rest[end..];
            continue;
        }

        // Identifier / number / other
        let ch = rest.chars().next().unwrap();
        if ch.is_ascii_alphabetic() || ch == '_' {
            let end = rest
                .char_indices()
                .skip(1)
                .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
                .map(|(i, _)| i)
                .unwrap_or(rest.len());
            let word = &rest[..end];
            let color = if keywords.contains(&word) {
                Color32::from_rgb(198, 120, 221)
            } else if apis.contains(&word) {
                Color32::from_rgb(229, 192, 123)
            } else {
                Color32::from_rgb(220, 223, 228)
            };
            job.append(
                word,
                0.0,
                egui::TextFormat {
                    font_id: FontId::monospace(13.5),
                    color,
                    ..Default::default()
                },
            );
            rest = &rest[end..];
        } else if ch.is_ascii_digit() {
            let end = rest
                .char_indices()
                .skip(1)
                .find(|(_, c)| !(c.is_ascii_digit() || *c == '.' || *c == '_'))
                .map(|(i, _)| i)
                .unwrap_or(rest.len());
            job.append(
                &rest[..end],
                0.0,
                egui::TextFormat {
                    font_id: FontId::monospace(13.5),
                    color: Color32::from_rgb(209, 154, 102),
                    ..Default::default()
                },
            );
            rest = &rest[end..];
        } else {
            let len = ch.len_utf8();
            job.append(
                &rest[..len],
                0.0,
                egui::TextFormat {
                    font_id: FontId::monospace(13.5),
                    color: Color32::from_rgb(170, 178, 190),
                    ..Default::default()
                },
            );
            rest = &rest[len..];
        }
    }

    ui.fonts(|f| f.layout_job(job))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_covers_host_api() {
        let names: Vec<_> = shiloh_script_catalog().iter().map(|s| s.name).collect();
        for need in [
            "on_ready",
            "on_update",
            "log",
            "spawn_named",
            "set_translation",
            "emit_signal",
            "play_audio",
        ] {
            assert!(names.contains(&need), "missing {need}");
        }
    }

    #[test]
    fn completion_replaces_prefix() {
        let mut ed = ScriptEditorState {
            buffer: "lo".into(),
            cursor_ccursor: 2,
            ..Default::default()
        };
        ed.recompute_completions();
        assert!(ed.completer_open);
        // Prefer `log`
        if let Some(i) = ed
            .completer_filtered
            .iter()
            .position(|&ci| shiloh_script_catalog()[ci].name == "log")
        {
            ed.completer_index = i;
        }
        ed.apply_completion();
        assert!(ed.buffer.starts_with("log"));
    }
}
