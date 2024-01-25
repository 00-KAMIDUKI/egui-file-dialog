use std::{fs, io};
use std::path::{Path, PathBuf};

use directories::UserDirs;
use sysinfo::Disks;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DialogMode {
    SelectFile,
    SelectDirectory,
    SaveFile
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum DialogState {
    Open,
    Closed,
    Selected(PathBuf),
    Cancelled
}

pub struct FileDialog {
    mode: DialogMode,
    state: DialogState,
    initial_directory: PathBuf,

    user_directories: Option<UserDirs>,
    system_disks: Disks,

    directory_stack: Vec<PathBuf>,
    directory_offset: usize,
    directory_content: Vec<PathBuf>,

    create_directory_dialog: CreateDirectoryDialog,

    selected_item: Option<PathBuf>,
    file_name_input: String,  // Only used when mode = DialogMode::SaveFile
    file_name_input_error: Option<String>,

    scroll_to_selection: bool,
    search_value: String
}

impl Default for FileDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl FileDialog {
    pub fn new() -> Self {
        FileDialog {
            mode: DialogMode::SelectDirectory,
            state: DialogState::Closed,
            initial_directory: std::env::current_dir().unwrap_or_default(),

            user_directories: UserDirs::new(),
            system_disks: Disks::new_with_refreshed_list(),

            directory_stack: vec![],
            directory_offset: 0,
            directory_content: vec![],

            create_directory_dialog: CreateDirectoryDialog::new(),

            selected_item: None,
            file_name_input: String::new(),
            file_name_input_error: None,

            scroll_to_selection: false,
            search_value: String::new()
        }
    }

    pub fn initial_directory(mut self, directory: PathBuf) -> Self {
        self.initial_directory = directory.clone();
        self
    }

    pub fn open(&mut self, mode: DialogMode) {
        self.reset();

        self.mode = mode;
        self.state = DialogState::Open;

        // TODO: Error handling
        let _ = self.load_directory(&self.initial_directory.clone());
    }

    pub fn select_directory(&mut self) {
        self.open(DialogMode::SelectDirectory);
    }

    pub fn select_file(&mut self) {
        self.open(DialogMode::SelectFile);
    }

    pub fn save_file(&mut self) {
        self.open(DialogMode::SaveFile);
    }

    pub fn mode(&self) -> DialogMode {
        self.mode
    }

    pub fn state(&self) -> DialogState {
        self.state.clone()
    }

    pub fn update(&mut self, ctx: &egui::Context) -> &Self {
        if self.state != DialogState::Open {
            return self;
        }

        let mut is_open = true;

        egui::Window::new("File dialog")
            .open(&mut is_open)
            .default_size([800.0, 500.0])
            .collapsible(false)
            .show(ctx, |ui| {
                egui::TopBottomPanel::top("fe_top_panel")
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        self.ui_update_top_panel(ctx, ui);
                    });

                egui::SidePanel::left("fe_left_panel")
                    .resizable(true)
                    .default_width(150.0)
                    .width_range(100.0..=400.0)
                    .show_inside(ui, |ui| {
                        self.update_left_panel(ctx, ui);
                    });

                egui::TopBottomPanel::bottom("fe_bottom_panel")
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        self.update_bottom_panel(ctx, ui);
                    });

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    self.ui_update_central_panel(ui);
                });
            });

        // User closed the window without finishing the dialog
        if !is_open {
            self.cancel();
        }

        self
    }

    fn ui_update_top_panel(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        const NAV_BUTTON_SIZE: egui::Vec2 = egui::Vec2::new(25.0, 25.0);
        const SEARCH_INPUT_WIDTH: f32 = 120.0;

        ui.horizontal(|ui| {

            // Navigation buttons
            if let Some(x) = self.current_directory() {
                if ui_button_sized(ui, NAV_BUTTON_SIZE, "⏶", x.parent().is_some()) {
                    let _ = self.load_parent();
                }
            }
            else {
                let _ = ui_button_sized(ui, NAV_BUTTON_SIZE, "⏶", false);
            }

            if ui_button_sized(ui, NAV_BUTTON_SIZE, "⏴",
                               self.directory_offset + 1 < self.directory_stack.len()) {
                let _ = self.load_previous_directory();
            }

            if ui_button_sized(ui, NAV_BUTTON_SIZE, "⏵", self.directory_offset != 0) {
                let _ = self.load_next_directory();
            }

            if ui_button_sized(ui, NAV_BUTTON_SIZE, "+", !self.create_directory_dialog.is_open()) {
                if let Some(x) = self.current_directory() {
                    self.create_directory_dialog.open(x.to_path_buf());
                }
            }

            // Current path display
            egui::Frame::default()
                .stroke(egui::Stroke::new(2.0, ctx.style().visuals.window_stroke.color))
                .inner_margin(egui::Margin::symmetric(4.0, 4.0))
                .rounding(egui::Rounding::from(5.0))
                .show(ui, |ui| {
                    // TODO: Enable scrolling with mouse wheel
                    egui::ScrollArea::horizontal()
                        .auto_shrink([false, false])
                        .stick_to_right(true)
                        // TODO: Dynamically size scroll area to available width
                        .max_width(500.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.style_mut().spacing.item_spacing.x /= 2.5;

                                let mut path = PathBuf::new();
                                
                                if let Some(data) = self.current_directory() {
                                    for (i, segment) in data.iter().enumerate() {
                                        path.push(segment);

                                        if i != 0 {
                                            ui.label(">");
                                        }

                                        // TODO: Maybe use selectable_label instead of button?
                                        // TODO: Write current directory (last item) in bold text
                                        if ui.button(segment.to_str().unwrap_or("<ERR>"))
                                            .clicked() {
                                                let _ = self.load_directory(path.as_path());
                                                return;
                                        }
                                    }
                                }
                            });
                        });
                });

            // Reload button
            if ui.add_sized(NAV_BUTTON_SIZE, egui::Button::new("⟲")).clicked() {
                self.refresh();
            }

            // Search bar
            egui::Frame::default()
                .stroke(egui::Stroke::new(2.0, ctx.style().visuals.window_stroke.color))
                .inner_margin(egui::Margin::symmetric(4.0, 4.0))
                .rounding(egui::Rounding::from(5.0))
                .show(ui, |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                        ui.add_space(ctx.style().spacing.item_spacing.y);
                        ui.label("🔍");
                        ui.add_sized(egui::Vec2::new(SEARCH_INPUT_WIDTH, 0.0),
                                    egui::TextEdit::singleline(&mut self.search_value));
                    });
                });
        });

        ui.add_space(ctx.style().spacing.item_spacing.y);
    }

    fn update_left_panel(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
            self.ui_update_user_directories(ui);

            ui.add_space(ctx.style().spacing.item_spacing.y * 4.0);

            self.ui_update_devices(ui);
        });
    }

    fn update_bottom_panel(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        const BUTTON_SIZE: egui::Vec2 = egui::Vec2::new(78.0, 20.0);

        ui.add_space(5.0);

        ui.horizontal(|ui| {
            match &self.mode {
                DialogMode::SelectDirectory => ui.label("Selected directory:"),
                DialogMode::SelectFile => ui.label("Selected file:"),
                DialogMode::SaveFile => ui.label("File name:")
            };

            match &self.mode {
                DialogMode::SelectDirectory | DialogMode::SelectFile => {
                    if self.is_selection_valid() {
                        if let Some(x) = &self.selected_item {
                            if let Some(name) = self.get_file_name(x) {
                                ui.colored_label(ui.style().visuals.selection.bg_fill, name);
                            }
                        }
                    }
                },
                DialogMode::SaveFile => {
                    let response = ui.add(egui::TextEdit::singleline(&mut self.file_name_input));

                    if response.changed() {
                        self.file_name_input_error = self.validate_file_name_input();
                    }

                    if let Some(x) = &self.file_name_input_error {
                        // TODO: Use error icon instead
                        ui.label(x);
                    }
                }
            };
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            let label = match &self.mode {
                DialogMode::SelectDirectory | DialogMode::SelectFile => "Open",
                DialogMode::SaveFile => "Save"
            };

            if ui_button_sized(ui, BUTTON_SIZE, label, self.is_selection_valid()) {
                match &self.mode {
                    DialogMode::SelectDirectory | DialogMode::SelectFile => {
                        // self.selected_item should always contain a value,
                        // since self.is_selection_valid() validates the selection and
                        // returns false if the selection is none.
                        if let Some(selection) = self.selected_item.clone() {
                            self.finish(selection);
                        }
                    },
                    DialogMode::SaveFile => {
                        // self.current_directory should always contain a value,
                        // since self.is_selection_valid() makes sure there is no
                        // file_name_input_error. The file_name_input_error
                        // gets validated every time something changes
                        // by the validate_file_name_input, which sets an error
                        // if we are currently not in a directory.
                        if let Some(path) = self.current_directory() {
                            let mut full_path = path.to_path_buf();
                            full_path.push(&self.file_name_input);

                            self.finish(full_path);
                        }
                    }
                }
            }

            ui.add_space(ctx.style().spacing.item_spacing.y);

            if ui.add_sized(BUTTON_SIZE, egui::Button::new("Abort")).clicked() {
                self.cancel();
            }
        });
    }

    fn ui_update_central_panel(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
            egui::containers::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                // Temporarily take ownership of the directory contents to be able to
                // update it in the for loop using load_directory.
                // Otherwise we would get an error that `*self` cannot be borrowed as mutable
                // more than once at a time.
                // Make sure to return the function after updating the directory_content,
                // otherwise the change will be overwritten with the last statement of the function.
                let data = std::mem::take(&mut self.directory_content);

                for path in data.iter() {
                    let Some(file_name) = self.get_file_name(path) else { continue; };

                    if !self.search_value.is_empty() &&
                       !file_name.to_lowercase().contains(&self.search_value.to_lowercase()) {
                        continue;
                    }

                    let icon = match path.is_dir() {
                        true => "🗀",
                        _ => "🖹"
                    };

                    let mut selected = false;
                    if let Some(x) = &self.selected_item {
                        selected = x == path;
                    }

                    let response = ui.selectable_label(selected, format!("{} {}", icon, file_name));

                    if selected && self.scroll_to_selection {
                        response.scroll_to_me(None);
                        self.scroll_to_selection = false;
                    }

                    if response.clicked() {
                        self.select_item(path.as_path());
                    }

                    if response.double_clicked() {
                        if path.is_dir() {
                            let _ = self.load_directory(path);
                            return;
                        }

                        self.select_item(path.as_path());

                        if self.is_selection_valid() {
                            // self.selected_item should always contain a value
                            // since self.is_selection_valid() validates the selection
                            // and returns false if the selection is none.
                            if let Some(selection) = self.selected_item.clone() {
                                self.finish(selection);
                            }
                        }
                    }
                }

                self.directory_content = data;

                if let Some(dir) = self.create_directory_dialog.update(ui).directory() {
                    self.directory_content.push(dir.clone());
                    self.select_item(dir.as_path());
                }
            });
        });
    }

    fn ui_update_user_directories(&mut self, ui: &mut egui::Ui) {
        if let Some(dirs) = self.user_directories.clone() {
            ui.label("Places");

            if ui.selectable_label(self.current_directory() == Some(dirs.home_dir()),
                                   "🏠  Home").clicked() {
                let _ = self.load_directory(dirs.home_dir());
            }

            if let Some(path) = dirs.desktop_dir() {
                if ui.selectable_label(self.current_directory() == Some(path),
                                       "🖵  Desktop").clicked() {
                    let _ = self.load_directory(path);
                }
            }
            if let Some(path) = dirs.document_dir() {
                if ui.selectable_label(self.current_directory() == Some(path),
                                       "🗐  Documents").clicked() {
                    let _ = self.load_directory(path);
                }
            }
            if let Some(path) = dirs.download_dir() {
                if ui.selectable_label(self.current_directory() == Some(path),
                                       "📥  Downloads").clicked() {
                    let _ = self.load_directory(path);
                }
            }
            if let Some(path) = dirs.audio_dir() {
                if ui.selectable_label(self.current_directory() == Some(path),
                                       "🎵  Audio").clicked() {
                    let _ = self.load_directory(path);
                }
            }
            if let Some(path) = dirs.picture_dir() {
                if ui.selectable_label(self.current_directory() == Some(path),
                                       "🖼  Pictures").clicked() {
                    let _ = self.load_directory(path);
                }
            }
            if let Some(path) = dirs.video_dir() {
                if ui.selectable_label(self.current_directory() == Some(path),
                                       "🎞  Videos").clicked() {
                    let _ = self.load_directory(path);
                }
            }
        }
    }

    fn ui_update_devices(&mut self, ui: &mut egui::Ui) {
        ui.label("Devices");

        let disks = std::mem::take(&mut self.system_disks);

        for disk in &disks {
            // TODO: Get display name of the devices.
            // Currently on linux "/dev/sda1" is returned.
            let name = match disk.name().to_str() {
                Some(x) => x,
                None => continue
            };

            if ui.selectable_label(false, format!("🖴  {}", name)).clicked() {
                let _ = self.load_directory(disk.mount_point());
            }
        }

        self.system_disks = disks;
    }

    fn reset(&mut self) {
        self.state = DialogState::Closed;

        self.user_directories = UserDirs::new();
        self.system_disks = Disks::new_with_refreshed_list();

        self.directory_stack = vec![];
        self.directory_offset = 0;
        self.directory_content = vec![];

        self.create_directory_dialog = CreateDirectoryDialog::new();

        self.selected_item = None;
        self.file_name_input = String::new();
        self.scroll_to_selection = false;
        self.search_value = String::new();
    }

    fn refresh(&mut self) {
        self.user_directories = UserDirs::new();
        self.system_disks = Disks::new_with_refreshed_list();

        let _ = self.reload_directory();
    }

    fn finish(&mut self, selected_item: PathBuf) {
        self.state = DialogState::Selected(selected_item);
    }

    fn cancel(&mut self) {
        self.state = DialogState::Cancelled;
    }

    fn current_directory(&self) -> Option<&Path> {
        if let Some(x) = self.directory_stack.iter().nth_back(self.directory_offset) {
            return Some(x.as_path())
        }

        None
    }

    fn get_file_name(&self, file: &Path) -> Option<String> {
        if let Some(x) = file.file_name() {
            if let Some(x) = x.to_str() {
                return Some(x.to_string());
            }
        }

        None
    }

    fn is_selection_valid(&self) -> bool {
        if let Some(selection) = &self.selected_item {
            let file_name = self.get_file_name(selection);

            return match &self.mode {
                DialogMode::SelectDirectory => selection.is_dir() && file_name.is_some(),
                DialogMode::SelectFile => selection.is_file() && file_name.is_some(),
                DialogMode::SaveFile => self.file_name_input_error.is_none()
            };
        }

        if self.mode == DialogMode::SaveFile && self.file_name_input_error.is_none() {
            return true;
        }

        false
    }

    fn validate_file_name_input(&self) -> Option<String> {
        if self.file_name_input.is_empty() {
            return Some("The file name cannot be empty".to_string());
        }

        if let Some(x) = self.current_directory() {
            let mut full_path = x.to_path_buf();
            full_path.push(self.file_name_input.as_str());

            if full_path.exists() && full_path.is_file() {
                return Some("A file with this name already exists".to_string());
            }
        }
        else {
            // There is most likely a bug in the code if we get this error message!
            return Some("Currently not in a directory".to_string())
        }

        None
    }

    fn select_item(&mut self, path: &Path) {
        self.selected_item = Some(path.to_path_buf());

        if self.mode == DialogMode::SaveFile && path.is_file() {
            if let Some(file_name) = self.get_file_name(path) {
                self.file_name_input = file_name;
                self.file_name_input_error = self.validate_file_name_input();
            }
        }
    }

    fn load_next_directory(&mut self) -> io::Result<()> {
        if self.directory_offset == 0 {
            // There is no next directory that can be loaded
            return Ok(());
        }

        self.directory_offset -= 1;

        // Copy path and load directory
        let path = self.current_directory().unwrap().to_path_buf();
        self.load_directory_content(path.as_path())
    }

    fn load_previous_directory(&mut self) -> io::Result<()> {
        if self.directory_offset + 1 >= self.directory_stack.len() {
            // There is no previous directory that can be loaded
            return Ok(())
        }

        self.directory_offset += 1;
    
        // Copy path and load directory
        let path = self.current_directory().unwrap().to_path_buf();
        self.load_directory_content(path.as_path())
    }

    fn load_parent(&mut self) -> io::Result<()> {
        if let Some(x) = self.current_directory() {
            if let Some(x) = x.to_path_buf().parent() {
                return self.load_directory(x);
            }
        }

        Ok(())
    }

    fn reload_directory(&mut self) -> io::Result<()> {
        if let Some(x) = self.current_directory() {
            return self.load_directory_content(x.to_path_buf().as_path());
        }

        Ok(())
    }

    fn load_directory(&mut self, path: &Path) -> io::Result<()> {
        // Do not load the same directory again.
        // Use reload_directory if the content of the directory should be updated.
        if let Some(x) = self.current_directory() {
            if x == path {
                return Ok(());
            }
        }

        if self.directory_offset != 0 && self.directory_stack.len() > self.directory_offset {
            self.directory_stack.drain(self.directory_stack.len() - self.directory_offset..);
        }

        self.directory_stack.push(fs::canonicalize(path)?);
        self.directory_offset = 0;

        self.load_directory_content(path)
    }

    fn load_directory_content(&mut self, path: &Path) -> io::Result<()> {
        let paths = fs::read_dir(path)?;

        self.create_directory_dialog.close();
        self.directory_content.clear();
        self.scroll_to_selection = true;

        for path in paths {
            match path {
                Ok(entry) => self.directory_content.push(entry.path()),
                _ => continue
            };
        }

        // TODO: Sort content to display folders first
        // TODO: Implement "Show hidden files and folders" option

        if self.mode == DialogMode::SaveFile {
            self.file_name_input_error = self.validate_file_name_input();
        }

        Ok(())
    }
}

struct CreateDirectoryResponse {
    directory: Option<PathBuf>
}

impl CreateDirectoryResponse {
    pub fn new(directory: &Path) -> Self {
        Self {
            directory: Some(directory.to_path_buf())
        }
    }

    pub fn new_empty() -> Self {
        Self {
            directory: None
        }
    }

    pub fn directory(&self) -> Option<PathBuf> {
        self.directory.clone()
    }
}

struct CreateDirectoryDialog {
    open: bool,
    init: bool,
    directory: Option<PathBuf>,

    input: String,
    error: Option<String>
}

impl CreateDirectoryDialog {
    pub fn new() -> Self {
        Self {
            open: false,
            init: false,
            directory: None,

            input: String::new(),
            error: None
        }
    }

    pub fn open(&mut self, directory: PathBuf) {
        self.reset();

        self.open = true;
        self.init = true;
        self.directory = Some(directory);
    }

    pub fn close(&mut self) {
        self.reset();
    }

    pub fn update(&mut self, ui: &mut egui::Ui) -> CreateDirectoryResponse {

        if !self.open {
            return CreateDirectoryResponse::new_empty();
        }

        let mut result = CreateDirectoryResponse::new_empty();

        ui.horizontal(|ui| {
            ui.label("🗀");

            let response = ui.text_edit_singleline(&mut self.input);

            if self.init {
                response.scroll_to_me(None);
                response.request_focus();

                self.error = self.validate_input();
                self.init = false;
            }

            if response.changed() {
                self.error = self.validate_input();
            }

            if ui_button(ui, "✔", self.error.is_none()) {
                result = self.create_directory();
            }

            if ui.button("✖").clicked() {
                self.close();
            }

            if let Some(err) = &self.error {
                // TODO: Use error icon instead
                ui.label(err);
            }
        });

        result
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    fn create_directory(&mut self) -> CreateDirectoryResponse {
        if let Some(mut dir) = self.directory.clone() {
            dir.push(self.input.as_str());

            match fs::create_dir(&dir) {
                Ok(()) => {
                    self.close();
                    return CreateDirectoryResponse::new(dir.as_path());
                }
                Err(err) => {
                    self.error = Some(format!("Error: {}", err));
                    return CreateDirectoryResponse::new_empty();
                }
            }
        }

        // This error should not occur because the create_directory function is only
        // called when the dialog is open and the directory is set.
        // If this error occurs, there is most likely a bug in the code.
        self.error = Some("No directory given".to_string());

        CreateDirectoryResponse::new_empty()
    }

    fn validate_input(&mut self) -> Option<String> {
        if self.input.is_empty() {
            return Some("Name of the folder can not be empty".to_string());
        }

        if let Some(mut x) = self.directory.clone() {
            x.push(self.input.as_str());

            if x.is_dir() {
                return Some("A directory with the name already exists".to_string())
            }
        }
        else {
            // This error should not occur because the validate_input function is only
            // called when the dialog is open and the directory is set.
            // If this error occurs, there is most likely a bug in the code.
            return Some("No directory given".to_string())
        }

        None
    }

    fn reset(&mut self) {
        self.open = false;
        self.init = false;
        self.directory = None;
        self.input.clear();
    }
}

#[inline]
fn get_disabled_fill_color(ui: &egui::Ui) -> egui::Color32 {
    let c = ui.style().visuals.widgets.noninteractive.bg_fill;
    egui::Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 100)
}

fn ui_button(ui: &mut egui::Ui, text: &str, enabled: bool) -> bool {
    if !enabled {
        let button = egui::Button::new(text)
            .stroke(egui::Stroke::NONE)
            .fill(get_disabled_fill_color(ui));

        let _ = ui.add(button);

        return false;
    }

    ui.add(egui::Button::new(text)).clicked()
}

fn ui_button_sized(ui: &mut egui::Ui, size: egui::Vec2, text: &str, enabled: bool) -> bool {
    if !enabled {
        let button = egui::Button::new(text)
            .stroke(egui::Stroke::NONE)
            .fill(get_disabled_fill_color(ui));

        let _ = ui.add_sized(size, button);

        return false;
    }

    ui.add_sized(size, egui::Button::new(text)).clicked()
}
