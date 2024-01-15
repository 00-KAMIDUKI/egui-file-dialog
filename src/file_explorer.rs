use std::{fs, io};
use directories::UserDirs;

pub struct FileExplorer {
    directory_content: Vec<fs::DirEntry>,
    user_directories: Option<UserDirs>,
    search_value: String
}

impl Default for FileExplorer {
    fn default() -> Self {
        Self::new()
    }
}

impl FileExplorer {
    pub fn new() -> Self {
        FileExplorer {
            directory_content: vec![],
            user_directories: UserDirs::new(),
            search_value: String::new() }
    }

    // TODO: Enable option to set initial directory
    pub fn open(&mut self) {
        // TODO: Error handling
        let _ = self.load_directory("./");
    }

    pub fn update(&mut self, ctx: &egui::Context) {
        // TODO: Make window title and options configurable
        egui::Window::new("File explorer")
            .default_size([800.0, 500.0])
            .show(ctx, |ui| {
                egui::TopBottomPanel::top("fe_top_panel")
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        self.update_top_panel(ctx, ui);
                    });

                egui::SidePanel::left("fe_left_panel")
                    .resizable(true)
                    .default_width(150.0)
                    .width_range(80.0..=300.0)
                    .show_inside(ui, |ui| {
                        self.update_left_panel(ctx, ui);
                    });

                egui::TopBottomPanel::bottom("fe_bottom_panel")
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        self.update_bottom_panel(ctx, ui);
                    });

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    self.update_central_panel(ui);
                });
            });
    }

    fn update_top_panel(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        const NAV_BUTTON_SIZE: egui::Vec2 = egui::Vec2::new(25.0, 25.0);

        ui.horizontal(|ui| {
            // Navigation buttons
            let _ = ui.add_sized(NAV_BUTTON_SIZE, egui::Button::new("<-"));
            let _ = ui.add_sized(NAV_BUTTON_SIZE, egui::Button::new("<"));
            let _ = ui.add_sized(NAV_BUTTON_SIZE, egui::Button::new(">"));
            let _ = ui.add_sized(NAV_BUTTON_SIZE, egui::Button::new("+"));

            // Current path display
            egui::Frame::default()
                .stroke(egui::Stroke::new(2.0, ctx.style().visuals.window_stroke.color))
                .inner_margin(egui::Margin::symmetric(4.0, 4.0))
                .rounding(egui::Rounding::from(5.0))
                .show(ui, |ui| {
                    // TODO: Set scroll area width to available width
                    egui::ScrollArea::horizontal()
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // NOTE: These are currently only hardcoded test values!
                                let _ = ui.add_sized(egui::Vec2::new(0.0, ui.available_height()),
                                                    egui::Button::new("home"));
                                ui.label(">");

                                let _ = ui.add_sized(egui::Vec2::new(0.0, ui.available_height()),
                                                    egui::Button::new("user"));
                                ui.label(">");

                                let _ = ui.add_sized(egui::Vec2::new(0.0, ui.available_height()),
                                                    egui::Button::new("documents"));
                                ui.label(">");

                                let _ = ui.add_sized(egui::Vec2::new(0.0, ui.available_height()),
                                                    egui::Button::new("projects"));
                        });
                    });
                });

            egui::Frame::default()
                .stroke(egui::Stroke::new(2.0, ctx.style().visuals.window_stroke.color))
                .inner_margin(egui::Margin::symmetric(4.0, 4.0))
                .rounding(egui::Rounding::from(5.0))
                .show(ui, |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        ui.add_space(ctx.style().spacing.item_spacing.y);
                        ui.label("🔍");
                        ui.add_sized(egui::Vec2::new(120.0, ui.available_height()),
                                     egui::TextEdit::singleline(&mut self.search_value));
                    });
                });
        });

        ui.add_space(ctx.style().spacing.item_spacing.y);
    }

    fn update_left_panel(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        self.update_user_directories(ui);

        ui.add_space(ctx.style().spacing.item_spacing.y * 4.0);

        ui.label("Devices");

        let _ = ui.selectable_label(false, "🖴  (C:)");
        let _ = ui.selectable_label(false, "🖴  Toshiba(D:)");
        let _ = ui.selectable_label(false, "🖴  Samsung 980..(E:)");
        let _ = ui.selectable_label(false, "🖴  (F:)");
    }

    fn update_bottom_panel(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        const BUTTON_SIZE: egui::Vec2 = egui::Vec2::new(78.0, 20.0);

        ui.add_space(5.0);

        ui.horizontal(|ui|{
            ui.label("Selected item: Desktop");

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                let _ = ui.add_sized(BUTTON_SIZE, egui::Button::new("Open"));
                ui.add_space(ctx.style().spacing.item_spacing.y);
                let _ = ui.add_sized(BUTTON_SIZE, egui::Button::new("Abort"));
            });
        });
    }

    fn update_central_panel(&mut self, ui: &mut egui::Ui) {
        for item in self.directory_content.iter() {
            let path = item.path();

            let icon = match path.is_dir() {
                true => "🗀",
                _ => "🖹"
            };

            // Is there a way to write this better?
            let file_name = match path.file_name() {
                Some(x) => {
                    match x.to_str() {
                        Some(v) => v,
                        _ => continue
                    }
                },
                _ => continue
            };

            let _ = ui.selectable_label(false, format!("{} {}", icon, file_name));
        }
    }

    fn update_user_directories(&mut self, ui: &mut egui::Ui) {
        if let Some(dirs) = &self.user_directories {
            ui.label("Places");

            let _ = ui.selectable_label(false, "🏠  Home");

            if dirs.document_dir().is_some() {
                let _ = ui.selectable_label(false, "🗐  Documents");
            }
            if dirs.download_dir().is_some() {
                let _ = ui.selectable_label(false, "📥  Downloads");
            }
            if dirs.audio_dir().is_some() {
                let _ = ui.selectable_label(false, "🎵  Audio");
            }
            if dirs.picture_dir().is_some() {
                let _ = ui.selectable_label(false, "🖼  Pictures");
            }
            if dirs.video_dir().is_some() {
                let _ = ui.selectable_label(false, "🎞  Videos");
            }
        }
    }

    fn load_directory(&mut self, path: &str) -> io::Result<()> {
        let paths = fs::read_dir(path)?;

        self.directory_content.clear();
        for path in paths {
            match path {
                Ok(entry) => self.directory_content.push(entry),
                _ => continue
            };
        }

        Ok(())
    }
}
