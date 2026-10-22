use crate::config::Config;
use std::path::PathBuf;
use std::sync::mpsc;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIconBuilder};

#[derive(Debug, PartialEq)]
enum Line<'a> {
    Plain(&'a str),
    Heading(usize, &'a str),
    Unordered(usize, &'a str),
}

#[allow(non_snake_case)]
fn parseLine(line: &str) -> Line<'_> {
    let headings = line.bytes().take_while(|&byte| byte == b'#').count();
    if (1..=6).contains(&headings) {
        if let Some(text) = line[headings..].strip_prefix(' ') {
            return Line::Heading(headings, text);
        }
    }
    let spaces = line.bytes().take_while(|&byte| byte == b' ').count();
    if matches!(spaces, 0 | 2 | 4) {
        for marker in ["- ", "* ", "+ "] {
            if let Some(text) = line[spaces..].strip_prefix(marker) {
                return Line::Unordered(spaces / 2 + 1, text);
            }
        }
    }
    Line::Plain(line)
}

struct TextLayout {
    _lines: Vec<(egui::Pos2, std::sync::Arc<egui::Galley>)>,
    _size: egui::Vec2,
}

#[allow(non_snake_case)]
fn layoutText(
    ctx: &egui::Context,
    text: &str,
    font: &egui::FontId,
    color: egui::Color32,
) -> TextLayout {
    let mut layout = TextLayout {
        _lines: Vec::new(),
        _size: egui::Vec2::ZERO,
    };
    let lines: Vec<&str> = if text.is_empty() {
        vec![""]
    } else {
        text.lines().collect()
    };
    for line in lines {
        let (text, scale, indent) = match parseLine(line) {
            Line::Plain(text) => (text.to_owned(), 1.0, 0.0),
            Line::Heading(level, text) => (
                text.to_owned(),
                [2.0, 1.8, 1.6, 1.4, 1.2, 1.1][level - 1],
                0.0,
            ),
            Line::Unordered(level, text) => {
                (format!("\u{2022} {text}"), 1.0, font.size * level as f32)
            }
        };
        let font = egui::FontId::new(font.size * scale, font.family.clone());
        let (galley, height) = ctx.fonts_mut(|fonts| {
            let height = fonts.row_height(&font);
            let galley = fonts.layout_no_wrap(text, font, color);
            let height = height.max(galley.size().y);
            (galley, height)
        });
        layout._size.x = layout._size.x.max(indent + galley.size().x);
        layout
            ._lines
            .push((egui::pos2(indent, layout._size.y), galley));
        layout._size.y += height;
    }
    layout
}

#[allow(non_snake_case)]
fn handleTrayEvent(
    ctx: &egui::Context,
    event: MenuEvent,
    toggleItem: &MenuItem,
    exitId: &tray_icon::menu::MenuId,
    visible: &mut bool,
) {
    if event.id == *exitId {
        ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
    } else if event.id == *toggleItem.id() {
        *visible = !*visible;
        toggleItem.set_text(if *visible { "Hide" } else { "Show" });
        ctx.send_viewport_cmd_to(
            egui::ViewportId::ROOT,
            egui::ViewportCommand::Visible(*visible),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(non_snake_case)]
    fn trayTogglesVisibilityAndExitsWhileHidden() {
        let ctx = egui::Context::default();
        let toggleItem = MenuItem::new("Hide", true, None);
        let exitItem = MenuItem::new("Exit", true, None);
        let mut visible = true;
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
        for expected in [false, true, false] {
            handleTrayEvent(
                &ctx,
                MenuEvent {
                    id: toggleItem.id().clone(),
                },
                &toggleItem,
                exitItem.id(),
                &mut visible,
            );
            assert_eq!(visible, expected);
            assert_eq!(toggleItem.text(), if expected { "Hide" } else { "Show" });
            let output = ctx.run_ui(egui::RawInput::default(), |_| {});
            assert!(output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&egui::ViewportCommand::Visible(expected)));
        }
        handleTrayEvent(
            &ctx,
            MenuEvent {
                id: exitItem.id().clone(),
            },
            &toggleItem,
            exitItem.id(),
            &mut visible,
        );
        let output = ctx.run_ui(egui::RawInput::default(), |_| {});
        assert!(output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::Close));
        assert!(!visible);
        let unrelatedItem = MenuItem::new("Unrelated", true, None);
        handleTrayEvent(
            &ctx,
            MenuEvent {
                id: unrelatedItem.id().clone(),
            },
            &toggleItem,
            exitItem.id(),
            &mut visible,
        );
        let output = ctx.run_ui(egui::RawInput::default(), |_| {});
        assert!(output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .is_empty());
        assert!(!visible);
    }

    #[test]
    #[allow(non_snake_case)]
    fn parsesHeadings() {
        for level in 1..=6 {
            let input = format!("{} Title", "#".repeat(level));
            assert_eq!(parseLine(&input), Line::Heading(level, "Title"));
        }
        assert_eq!(parseLine("# "), Line::Heading(1, ""));
        assert_eq!(parseLine("##  Title #"), Line::Heading(2, " Title #"));
        assert_eq!(
            parseLine("# \u{6807}\u{9898}"),
            Line::Heading(1, "\u{6807}\u{9898}")
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn parsesLists() {
        for marker in ["-", "*", "+"] {
            assert_eq!(
                parseLine(&format!("{marker} Item")),
                Line::Unordered(1, "Item")
            );
        }
        assert_eq!(parseLine("- "), Line::Unordered(1, ""));
        assert_eq!(parseLine("-  Item"), Line::Unordered(1, " Item"));
        assert_eq!(parseLine("  - "), Line::Unordered(2, ""));
        assert_eq!(parseLine("    +  Item"), Line::Unordered(3, " Item"));
        for input in [
            "1. Item",
            "001. Item",
            "0. ",
            "999999999999999999999999999999. Item",
        ] {
            assert_eq!(parseLine(input), Line::Plain(input));
        }
    }

    #[test]
    #[allow(non_snake_case)]
    fn laysOutThreeListLevels() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let font = egui::FontId::proportional(16.0);
            let color = egui::Color32::WHITE;
            for marker in ["-", "*", "+"] {
                for level in 1..=3 {
                    let input = format!("{}{marker} Item", "  ".repeat(level - 1));
                    let layout = layoutText(ui.ctx(), &input, &font, color);
                    assert_eq!(layout._lines[0].1.text(), "\u{2022} Item");
                    assert_eq!(layout._lines[0].0.x, font.size * level as f32);
                    assert!(layout._size.x >= layout._lines[0].0.x + layout._lines[0].1.size().x);
                }
            }
            for input in ["  - ", "    +  Item"] {
                let layout = layoutText(ui.ctx(), input, &font, color);
                assert!(layout._lines[0].1.text().starts_with("\u{2022} "));
            }
        });
    }

    #[test]
    #[allow(non_snake_case)]
    fn laysOutMarkdown() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let ctx = ui.ctx();
            let font = egui::FontId::proportional(16.0);
            let color = egui::Color32::WHITE;
            let plain = layoutText(ctx, "Title", &font, color);
            let heading = layoutText(ctx, "# Title", &font, color);
            assert_eq!(heading._lines[0].1.text(), "Title");
            assert!(heading._size.y > plain._size.y);
            let mixed = layoutText(
                ctx,
                "# Title\r\n\r\n- Item\r\n001. Item\r\nText\r\n",
                &font,
                color,
            );
            assert_eq!(mixed._lines.len(), 5);
            assert_eq!(mixed._lines[2].1.text(), "\u{2022} Item");
            assert_eq!(mixed._lines[3].1.text(), "001. Item");
            assert_eq!(mixed._lines[2].0.x, font.size);
            assert_eq!(mixed._lines[3].0.x, 0.0);
            for pair in mixed._lines.windows(2) {
                assert!(pair[1].0.y >= pair[0].0.y + pair[0].1.size().y);
            }
            for (pos, galley) in &mixed._lines {
                assert!(pos.x + galley.size().x <= mixed._size.x);
                assert!(pos.y + galley.size().y <= mixed._size.y);
            }
            let empty = layoutText(ctx, "", &font, color);
            assert_eq!(empty._lines.len(), 1);
            assert!(empty._size.y > 0.0);
            let blanks = layoutText(ctx, "\n\n", &font, color);
            assert_eq!(blanks._lines.len(), 2);
            assert_eq!(blanks._size.y, empty._size.y * 2.0);
            let plain = layoutText(ctx, "  Text  ", &font, color);
            assert_eq!(plain._lines[0].1.text(), "  Text  ");
            assert_eq!(plain._lines[0].0.x, 0.0);
        });
    }

    #[test]
    #[allow(non_snake_case)]
    fn preservesPlainText() {
        for input in [
            "",
            "Text",
            "  Text  ",
            "#Title",
            "####### Title",
            " # Title",
            "-\tItem",
            "-Item",
            " - Item",
            "   - Item",
            "     - Item",
            "      - Item",
            "        * Item",
            "\t- Item",
            "  \t- Item",
            "  1. Item",
            "  -Item",
            "    -\tItem",
            "1.Item",
            "1) Item",
            "\u{ff11}. Item",
            "**Bold**",
            "#\tTitle",
        ] {
            assert_eq!(parseLine(input), Line::Plain(input));
        }
    }
}

pub struct StickyNoteApp {
    config: Config,
    note_path: PathBuf,
    text: String,
    rx: mpsc::Receiver<()>,
    _watcher: notify::RecommendedWatcher,
    last_size: egui::Vec2,
    taskbar_hidden: bool,
}

impl StickyNoteApp {
    pub fn new(
        config: Config,
        note_path: PathBuf,
        text: String,
        rx: mpsc::Receiver<()>,
        watcher: notify::RecommendedWatcher,
        ctx: egui::Context,
    ) -> Self {
        let (ready_tx, ready_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let menu = Menu::new();
            let toggle_item = MenuItem::new("Hide", true, None);
            let exit_item = MenuItem::new("Exit", true, None);
            menu.append(&toggle_item)
                .expect("append visibility menu item");
            menu.append(&exit_item).expect("append exit menu item");

            let _tray_icon = TrayIconBuilder::new()
                .with_tooltip("StickyNote")
                .with_icon(make_tray_icon())
                .with_menu(Box::new(menu))
                .build()
                .expect("build tray icon");

            ready_tx.send(()).expect("send tray ready signal");
            run_message_loop(&ctx, &toggle_item, exit_item.id());
        });
        ready_rx.recv().expect("receive tray ready signal");

        Self {
            config,
            note_path,
            text,
            rx,
            _watcher: watcher,
            last_size: egui::Vec2::ZERO,
            taskbar_hidden: false,
        }
    }

    fn reload_text(&mut self) {
        if let Ok(content) = std::fs::read_to_string(&self.note_path) {
            self.text = content;
        }
    }

    fn font_id(&self) -> egui::FontId {
        egui::FontId::new(self.config.text.font_size, egui::FontFamily::Proportional)
    }
}

impl eframe::App for StickyNoteApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx();

        ctx.request_repaint_after(std::time::Duration::from_millis(100));

        if !self.taskbar_hidden {
            if let Some(window) = frame.winit_window() {
                use winit::platform::windows::WindowExtWindows;
                window.set_skip_taskbar(true);
                self.taskbar_hidden = true;
            }
        }

        while self.rx.try_recv().is_ok() {
            self.reload_text();
        }

        let font_id = self.font_id();
        let text_color = self.config.text_color();
        let bg_color = self.config.bg_color();
        let padding = &self.config.padding;

        let layout = layoutText(ctx, &self.text, &font_id, text_color);

        let target_w = layout._size.x + padding.left + padding.right;
        let target_h = layout._size.y + padding.top + padding.bottom;
        let target_size = egui::vec2(target_w.max(50.0), target_h.max(30.0));

        if (target_size - self.last_size).length() > 1.0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target_size));
            self.last_size = target_size;
        }

        let painter = ui.painter();
        let rect = ui.max_rect();

        painter.rect_filled(rect, self.config.corner_radius, bg_color);

        let origin = rect.min + egui::vec2(padding.left, padding.top);
        for (offset, galley) in layout._lines {
            painter.galley(origin + offset.to_vec2(), galley, text_color);
        }

        let response = ui.interact(rect, ui.id().with("drag_area"), egui::Sense::drag());
        let primary_pressed = ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
        if response.is_pointer_button_down_on() && primary_pressed {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        let secondary_clicked = ctx.input(|i| i.pointer.secondary_clicked());
        if secondary_clicked {
            open_editor(&self.note_path);
        }
    }
}

fn open_editor(path: &std::path::Path) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", "", &path.to_string_lossy()])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

#[cfg(target_os = "windows")]
fn run_message_loop(
    ctx: &egui::Context,
    toggle_item: &MenuItem,
    exit_id: &tray_icon::menu::MenuId,
) {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, TranslateMessage, MSG,
    };
    let mut visible = true;
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
            // 隐藏窗口不执行 App::ui；菜单分发结束后在所属线程更新菜单并唤醒窗口。
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                handleTrayEvent(ctx, event, toggle_item, exit_id, &mut visible);
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn run_message_loop(
    _ctx: &egui::Context,
    _toggle_item: &MenuItem,
    _exit_id: &tray_icon::menu::MenuId,
) {
}

fn make_tray_icon() -> Icon {
    const SIZE: u32 = 32;
    let radius = 5.0_f32;
    let half = SIZE as f32 / 2.0;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let dx = (px - half).abs() - (half - radius);
            let dy = (py - half).abs() - (half - radius);
            let inside = if dx > 0.0 && dy > 0.0 {
                (dx * dx + dy * dy).sqrt() <= radius
            } else {
                dx <= 0.0 || dy <= 0.0
            };
            if inside {
                rgba.extend_from_slice(&[255, 214, 90, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Icon::from_rgba(rgba, SIZE, SIZE).expect("create tray icon")
}
