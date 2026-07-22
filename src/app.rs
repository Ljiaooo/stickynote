use crate::config::Config;
use std::path::PathBuf;
use std::sync::mpsc;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIconBuilder};

pub struct StickyNoteApp {
    config: Config,
    note_path: PathBuf,
    text: String,
    rx: mpsc::Receiver<()>,
    _watcher: notify::RecommendedWatcher,
    last_size: egui::Vec2,
    exit_menu_id: tray_icon::menu::MenuId,
    taskbar_hidden: bool,
}

impl StickyNoteApp {
    pub fn new(
        config: Config,
        note_path: PathBuf,
        text: String,
        rx: mpsc::Receiver<()>,
        watcher: notify::RecommendedWatcher,
    ) -> Self {
        // 托盘图标放在独立线程中，运行自己的 Win32 消息循环。
        // winit 的事件循环不会分发托盘隐藏窗口的消息，
        // 因此必须在拥有托盘窗口的线程上泵送消息，事件才能到达全局 channel。
        let (id_tx, id_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let menu = Menu::new();
            let exit_item = MenuItem::new("Exit", true, None);
            let exit_id = exit_item.id().clone();
            menu.append(&exit_item).expect("append exit menu item");

            let _tray_icon = TrayIconBuilder::new()
                .with_tooltip("StickyNote")
                .with_icon(make_tray_icon())
                .with_menu(Box::new(menu))
                .build()
                .expect("build tray icon");

            debug_log(&format!(
                "tray created on thread {:?}, exit_id={:?}",
                std::thread::current().id(),
                exit_id
            ));

            id_tx.send(exit_id).expect("send exit menu id");
            run_message_loop();
        });
        let exit_menu_id = id_rx.recv().expect("receive exit menu id");

        Self {
            config,
            note_path,
            text,
            rx,
            _watcher: watcher,
            last_size: egui::Vec2::ZERO,
            exit_menu_id,
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
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx();

        // 定期唤醒以轮询托盘事件
        ctx.request_repaint_after(std::time::Duration::from_millis(100));

        debug_log(&format!(
            "ui frame on thread {:?}, taskbar_hidden={}",
            std::thread::current().id(),
            self.taskbar_hidden
        ));

        // 处理托盘菜单事件
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            debug_log(&format!("MenuEvent received: {:?}", event.id));
            if event.id == self.exit_menu_id {
                debug_log("Exit matched, sending Close");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        // 诊断：记录所有托盘事件
        while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
            debug_log(&format!("TrayIconEvent received: {:?}", event));
        }

        // 从任务栏隐藏窗口（仅一次）
        if !self.taskbar_hidden {
            if let Some(window) = frame.winit_window() {
                use winit::platform::windows::WindowExtWindows;
                window.set_skip_taskbar(true);
                self.taskbar_hidden = true;
            }
        }

        // 检查文件变化
        while self.rx.try_recv().is_ok() {
            self.reload_text();
        }

        let font_id = self.font_id();
        let text_color = self.config.text_color();
        let bg_color = self.config.bg_color();
        let padding = &self.config.padding;

        // 计算多行文本尺寸
        let lines: Vec<&str> = self.text.lines().collect();
        let lines = if lines.is_empty() { vec![""] } else { lines };

        let mut max_width: f32 = 0.0;
        let line_height = ctx.fonts_mut(|f| f.row_height(&font_id));

        for line in &lines {
            let galley = ctx.fonts_mut(|f| {
                f.layout_no_wrap((*line).to_owned(), font_id.clone(), text_color)
            });
            max_width = max_width.max(galley.size().x);
        }

        let total_height = line_height * lines.len() as f32;

        // 窗口目标尺寸
        let target_w = max_width + padding.left + padding.right;
        let target_h = total_height + padding.top + padding.bottom;
        let target_size = egui::vec2(target_w.max(50.0), target_h.max(30.0));

        // 尺寸变化超过阈值时才调整窗口
        if (target_size - self.last_size).length() > 1.0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target_size));
            self.last_size = target_size;
        }

        // 渲染
        let painter = ui.painter();
        let rect = ui.max_rect();

        // 圆角矩形背景
        painter.rect_filled(rect, self.config.corner_radius, bg_color);

        // 逐行绘制文本
        let mut y = rect.min.y + padding.top;
        for line in &lines {
            let pos = egui::pos2(rect.min.x + padding.left, y);
            painter.text(
                pos,
                egui::Align2::LEFT_TOP,
                *line,
                font_id.clone(),
                text_color,
            );
            y += line_height;
        }

        // 左键按下时启动窗口拖动
        let response = ui.interact(rect, ui.id().with("drag_area"), egui::Sense::drag());
        let primary_pressed = ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
        if response.is_pointer_button_down_on() && primary_pressed {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        // 右键打开外部编辑器
        let secondary_clicked = ctx.input(|i| i.pointer.secondary_clicked());
        if secondary_clicked {
            open_editor(&self.note_path);
        }
    }
}

fn open_editor(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", &path.to_string_lossy()])
            .spawn();
    }
}

// 在托盘线程上泵送 Win32 消息，使托盘隐藏窗口能收到并分发消息
#[cfg(target_os = "windows")]
fn run_message_loop() {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, TranslateMessage, MSG,
    };
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn run_message_loop() {}

// 临时调试日志：写入 stickynote_debug.log
fn debug_log(msg: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("stickynote_debug.log")
    {
        let _ = writeln!(f, "{msg}");
    }
}

// 生成 32x32 黄色圆角方块托盘图标
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
