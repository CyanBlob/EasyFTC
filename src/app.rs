use crate::app::generators::generator::Generator;
pub mod generators;

use generators::motors::dc_motor::DcMotor;
use generators::servos::rev_servo::RevServo;

use self::generators::generator::SubsystemGenerator;
use self::generators::subsystem::subsystem::Subsystem;
use self::theme::Theme;

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

pub mod syntax_highlighting;

pub mod theme;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    // Example stuff:
    label: String,

    drivetrain: Subsystem<DcMotor, RevServo>,
    subsystems: Vec<Subsystem<DcMotor, RevServo>>,
    code: String,

    #[serde(skip)]
    selected_subsystem: usize,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "EasyFTC".to_owned(),
            drivetrain: Subsystem::new("Drivetrain".to_owned(), true),
            subsystems: vec![],
            code: "".to_string(),
            selected_subsystem: 0,
        }
    }
}

impl TemplateApp {
    // Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        //cc.egui_ctx.set_visuals(egui::Visuals::light());

        let theme: Theme = Theme::new(&crate::config::AppStyle::default());
        cc.egui_ctx.set_visuals(theme.visuals);

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }

    pub fn generate_code(&mut self) {
        let mut new_code = String::new();

        // standard includes
        new_code += "package org.firstinspires.ftc.teamcode;\n\n";
        new_code += "import com.qualcomm.robotcore.eventloop.opmode.LinearOpMode;\n";
        new_code += "import com.qualcomm.robotcore.eventloop.opmode.TeleOp;\n";
        new_code += "import com.qualcomm.robotcore.util.ElapsedTime;\n";
        new_code += "import com.qualcomm.robotcore.util.Range;\n";

        // subsystem includes
        let mut includes = self.drivetrain.generate_includes().to_string();

        self.subsystems.iter().for_each(|subsystem| {
            includes += &subsystem.generate_includes().to_string();
        });

        // remove duplicate includes
        let mut includes_collection = includes.lines().collect::<Vec<&str>>();
        includes_collection.sort();
        includes_collection.dedup();

        for include in includes_collection {
            new_code += &include;
            new_code += "\n";
        }

        new_code += "\n";

        new_code += r#"@TeleOp(name="EasyFTC Teleop", group="Linear Opmode")"#;
        new_code += "\n";
        new_code += "public class EasyFTC_teleop extends LinearOpMode {\n\
                \n\tprivate ElapsedTime runtime = new ElapsedTime();\n\n";

        // global variables
        new_code += &self.drivetrain.generate_globals();

        self.subsystems.iter().for_each(|subsystem| {
            new_code += &subsystem.generate_globals();
        });

        new_code += "\t@Override\n\
        \tpublic void runOpMode() {\n\n\
            \t\ttelemetry.addData(\"Status\", \"Initialized\");\n\
            \t\ttelemetry.update();";
        new_code += "\n\n";

        // initializers
        new_code += &self.drivetrain.generate_init();

        self.subsystems.iter().for_each(|subsystem| {
            new_code += &subsystem.generate_init();
        });

        new_code += "\t\twaitForStart();\n\n\
            \t\t// Reset the timer (stopwatch) because we only care about time since the game\n\
            \t\t// actually starts\n\
            \t\truntime.reset();\n\n\
            \t\twhile (opModeIsActive()) {\n\n";

        // loop one-time setup
        new_code += &self.drivetrain.generate_loop_one_time_setup();

        self.subsystems.iter().for_each(|subsystem| {
            new_code += &subsystem.generate_loop_one_time_setup();
        });

        // loop
        new_code += &self.drivetrain.generate_loop();

        self.subsystems.iter().for_each(|subsystem| {
            new_code += &subsystem.generate_loop();
        });

        new_code += "\t\t\ttelemetry.update();\n\
                \t\t}\n\
            \t}\n}";

        self.code = new_code.to_string();
    }
}

impl eframe::App for TemplateApp {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut width: f32 = 0.0;

        //#[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            width = ui.available_width();

            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("This does nothing right now").clicked() {
                        //_frame.close();
                        //self.save(storage)
                    }
                });
            });
        });

        egui::SidePanel::right("code_panel").show(ctx, |ui| {
            if ui.button("Upload code").clicked() {
                let mut opt: ftc_http::Ftc = ftc_http::Ftc::default();
                opt.upload = true;

                let mut conf = ftc_http::AppConfig::default();

                match ftc_http::RobotController::new(&mut conf) {
                    Ok(r) => {
                        // create a tmp directory to write files into
                        let dir = tempfile::tempdir().unwrap();

                        // write teleop to a file
                        let file_path = dir.path().join("EasyFTC_teleop.java");
                        let mut tmpfile = File::create(&file_path).unwrap();

                        write!(tmpfile, "{}", &self.code).unwrap();

                        println!("Uploading files...");
                        match r.upload_files(vec![PathBuf::from(&file_path)]) {
                            Ok(_) => match r.build() {
                                Ok(_) => {
                                    println!("Build succeeded");
                                }
                                Err(_) => {
                                    println!("Build failed");
                                }
                            },
                            Err(_) => {
                                println!("Failed to upload files to robot");
                            }
                        }
                    }
                    Err(_) => {
                        println!("Error communicating with robot");
                    }
                };
            }

            ui.heading("Generated code");
            egui::scroll_area::ScrollArea::horizontal().show(ui, |ui| {
                egui::scroll_area::ScrollArea::vertical()
                    .auto_shrink([true; 2])
                    .show(ui, |ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                            show_code(ui, &self.code, ui.available_width());
                        });
                    });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::TopBottomPanel::top("subsystem_panel").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Subsystems: ");
                    if ui.button("Drivetrain").clicked() {
                        self.selected_subsystem = 0;
                    }

                    self.subsystems
                        .iter()
                        .enumerate()
                        .for_each(|(i, subsystem)| {
                            if ui.button(subsystem.get_name()).clicked() {
                                self.selected_subsystem = i + 1;
                            }
                        });

                    if ui.button("Add subsystem").clicked() {
                        self.subsystems.push(Subsystem::new(
                            format!("Subsystem_{}", self.subsystems.len() as i32 + 1),
                            false,
                        ));
                        self.selected_subsystem = self.subsystems.len();
                    }
                });
            });

            ui.add_space(30.0);

            if self.selected_subsystem == 0 {
                ui.heading("Drivetrain Configuration");
            } else {
                ui.heading(format!(
                    "{} Configuration",
                    self.subsystems
                        .iter()
                        .nth(self.selected_subsystem - 1)
                        .unwrap()
                        .get_name()
                ));

                ui.horizontal(|ui| {
                    let text_edit = egui::TextEdit::singleline(
                        &mut self
                            .subsystems
                            .iter_mut()
                            .nth(self.selected_subsystem - 1)
                            .unwrap()
                            .name,
                    )
                    .desired_width(100.0);
                    ui.add(text_edit);
                    ui.label("Rename subsystem");
                });

                ui.add_space(10.0);

                if ui.button("Delete subsystem").clicked() {
                    self.subsystems.remove(self.selected_subsystem - 1);
                    self.selected_subsystem -= 1;
                }
            }
            if self.selected_subsystem == 0 {
                self.drivetrain.render_options(ui, 0);
            } else {
                self.subsystems
                    .iter_mut()
                    .nth(self.selected_subsystem - 1)
                    .unwrap()
                    .render_options(ui, 0);
            }

            self.generate_code();
        });

        if false {
            egui::Window::new("Window").show(ctx, |ui| {
                ui.label("Windows can be moved by dragging them.");
                ui.label("They are automatically sized based on contents.");
                ui.label("You can turn on resizing and scrolling if you like.");
                ui.label("You would normally choose either panels OR windows.");
            });
        }
    }
}

fn show_code(ui: &mut egui::Ui, code: &str, width: f32) {
    let code = remove_leading_indentation(code.trim_start_matches('\n'));
    crate::app::syntax_highlighting::code_view_ui(ui, &code, width);
}

fn remove_leading_indentation(code: &str) -> String {
    fn is_indent(c: &u8) -> bool {
        matches!(*c, b' ' | b'\t')
    }

    let first_line_indent = code.bytes().take_while(is_indent).count();

    let mut out = String::new();

    let mut code = code;
    while !code.is_empty() {
        let indent = code.bytes().take_while(is_indent).count();
        let start = first_line_indent.min(indent);
        let end = code
            .find('\n')
            .map_or_else(|| code.len(), |endline| endline + 1);
        out += &code[start..end];
        code = &code[end..];
    }
    out
}
