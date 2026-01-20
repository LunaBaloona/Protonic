slint::include_modules!();
use device_query::{DeviceQuery, DeviceState, Keycode};
use rfd::FileDialog;
use rodio::{Decoder, OutputStream, Sink};
use serde::{Deserialize, Serialize};
use slint::{ModelRc, SharedString, VecModel};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{BufReader, Cursor};
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;
use steamlocate::SteamDir;

// Embed our ogg audio files at compile time
const AUDIO_LAUNCH_GAME: &[u8] = include_bytes!("../audio/LaunchGame.ogg");
const AUDIO_LAUNCH_PROGRAM: &[u8] = include_bytes!("../audio/LaunchProgram.ogg");

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct GameConfig {
    exe1_path: String,
    exe2_path: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct AppConfig {
    last_game_name: String,
    last_app_id: String,
    auto_configure: bool,
    game_configs: HashMap<String, GameConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            last_game_name: String::new(),
            last_app_id: String::new(),
            auto_configure: true,
            game_configs: HashMap::new(),
        }
    }
}

/// Play an embedded audio file in a separate thread
fn play_audio(audio_data: &'static [u8]) {
    thread::spawn(move || {
        // Get output stream - _stream must be kept alive for playback
        if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
            if let Ok(sink) = Sink::try_new(&stream_handle) {
                let cursor = Cursor::new(audio_data);
                if let Ok(source) = Decoder::new(BufReader::new(cursor)) {
                    sink.append(source);
                    sink.sleep_until_end();
                }
            }
        }
    });
}

/// Find the Steam userdata directory for the current user
fn find_steam_userdata_path() -> Option<PathBuf> {
    let steam_dir = SteamDir::locate().ok()?;
    let userdata_path = steam_dir.path().join("userdata");

    if userdata_path.exists() {
        // Find the first user directory (we're going to assume most users have only one)
        if let Ok(entries) = fs::read_dir(&userdata_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Check it's a numeric user ID directory
                    if let Some(name) = path.file_name() {
                        if name.to_string_lossy().chars().all(|c| c.is_ascii_digit()) {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Get the path to localconfig.vdf
fn get_localconfig_path() -> Option<PathBuf> {
    let userdata = find_steam_userdata_path()?;
    let localconfig = userdata.join("config").join("localconfig.vdf");
    if localconfig.exists() {
        Some(localconfig)
    } else {
        None
    }
}

/// Check if protonhax is already in the launch options for a game
fn has_protonhax_configured(app_id: &str) -> bool {
    if let Some(localconfig_path) = get_localconfig_path() {
        if let Ok(content) = fs::read_to_string(&localconfig_path) {
            // Simple text-based check - look for the app ID section and protonhax
            // VDF structure: "Apps" { "app_id" { "LaunchOptions" "..." } }
            if let Some(app_pos) = content.find(&format!("\"{}\"", app_id)) {
                // Look for LaunchOptions within a reasonable distance after the app ID
                let search_area = &content[app_pos..std::cmp::min(app_pos + 500, content.len())];
                if let Some(launch_pos) = search_area.find("\"LaunchOptions\"") {
                    let after_launch = &search_area[launch_pos..];
                    // Find the value (next quoted string after "LaunchOptions")
                    if let Some(first_quote) = after_launch[15..].find('"') {
                        let value_start = 15 + first_quote + 1;
                        if let Some(end_quote) = after_launch[value_start..].find('"') {
                            let launch_options =
                                &after_launch[value_start..value_start + end_quote];
                            return launch_options.contains("protonhax");
                        }
                    }
                }
            }
        }
    }
    false
}

/// Configure protonhax in Steam launch options for a game
fn configure_launch_options(app_id: &str) -> Result<String, String> {
    let localconfig_path =
        get_localconfig_path().ok_or_else(|| "Could not find Steam localconfig.vdf".to_string())?;

    let content = fs::read_to_string(&localconfig_path)
        .map_err(|e| format!("Failed to read localconfig.vdf: {}", e))?;

    // Check if already configured
    if has_protonhax_configured(app_id) {
        return Ok("Launch options already configured".to_string());
    }

    // Find the app section
    let app_pattern = format!("\"{}\"", app_id);
    let app_pos = content.find(&app_pattern).ok_or_else(|| {
        "Game not found in Steam config. Launch the game from Steam at least once first."
            .to_string()
    })?;

    // Find the opening brace for this app's section
    let after_app = &content[app_pos + app_pattern.len()..];
    let brace_offset = after_app
        .find('{')
        .ok_or_else(|| "Invalid VDF structure".to_string())?;
    let section_start = app_pos + app_pattern.len() + brace_offset + 1;

    // Check if LaunchOptions already exists
    let search_end = std::cmp::min(section_start + 500, content.len());
    let search_area = &content[section_start..search_end];

    let new_content = if let Some(launch_pos) = search_area.find("\"LaunchOptions\"") {
        // LaunchOptions exists - we need to prepend protonhax to the existing value
        let abs_launch_pos = section_start + launch_pos;
        let after_key = &content[abs_launch_pos + 15..]; // 15 = len of "LaunchOptions"

        // Find the value
        let first_quote = after_key
            .find('"')
            .ok_or_else(|| "Invalid LaunchOptions format".to_string())?;
        let value_start = abs_launch_pos + 15 + first_quote + 1;
        let value_area = &content[value_start..];
        let end_quote = value_area
            .find('"')
            .ok_or_else(|| "Invalid LaunchOptions format".to_string())?;

        let existing_options = &content[value_start..value_start + end_quote];

        // Build new launch options
        let new_options = if existing_options.is_empty() {
            "protonhax init %COMMAND%".to_string()
        } else {
            format!("protonhax init {} %COMMAND%", existing_options)
        };

        // Replace the old value with the new one
        format!(
            "{}{}{}",
            &content[..value_start],
            new_options,
            &content[value_start + end_quote..]
        )
    } else {
        // LaunchOptions doesn't exist - add it
        // Find a good place to insert (after the opening brace)
        let insert_pos = section_start;

        // Detect indentation by looking at the surrounding content
        let indent = "\t\t\t\t\t\t\t";
        let new_line = format!(
            "\n{}\"LaunchOptions\"\t\t\"protonhax init %COMMAND%\"",
            indent
        );

        format!(
            "{}{}{}",
            &content[..insert_pos],
            new_line,
            &content[insert_pos..]
        )
    };

    // Write the modified content back
    fs::write(&localconfig_path, new_content)
        .map_err(|e| format!("Failed to write localconfig.vdf: {}", e))?;

    Ok("Launch options configured successfully".to_string())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = AppWindow::new()?;

    // Load config from ~/.config/protonic/default-config.toml
    let cfg: AppConfig = confy::load("protonic", None).unwrap_or_default();

    // Use Arc<Mutex> for thread-safe config sharing
    let config = Arc::new(Mutex::new(cfg));

    // Set initial UI state from config
    {
        let cfg = config.lock().unwrap();
        ui.set_search_text(cfg.last_game_name.clone().into());
        ui.set_app_id(cfg.last_app_id.clone().into());
        ui.set_auto_configure(cfg.auto_configure);

        // Load exe paths for last selected game if any
        if !cfg.last_app_id.is_empty() {
            if let Some(game_cfg) = cfg.game_configs.get(&cfg.last_app_id) {
                ui.set_exe1_path(game_cfg.exe1_path.clone().into());
                ui.set_exe2_path(game_cfg.exe2_path.clone().into());
            }

            // Check launch options status
            if cfg.auto_configure {
                let status = if has_protonhax_configured(&cfg.last_app_id) {
                    "✓ Launch options configured"
                } else {
                    "Launch options will be configured on launch"
                };
                ui.set_launch_options_status(status.into());
            }
        }
    }

    // Fetch list of installed Steam games
    let mut games: BTreeMap<String, String> = BTreeMap::new();
    if let Ok(steam_dir) = SteamDir::locate() {
        if let Ok(library_iter) = steam_dir.libraries() {
            for library in library_iter {
                if let Ok(lib) = library {
                    for app in lib.apps() {
                        if let Ok(a) = app {
                            if let Some(name) = &a.name {
                                games.insert(name.clone(), a.app_id.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    let all_game_names: Vec<String> = games.keys().cloned().collect();

    // Initial population of the list (filtered by saved search text if any)
    let initial_search = config.lock().unwrap().last_game_name.to_lowercase();
    let initial_filtered: Vec<SharedString> = all_game_names
        .iter()
        .filter(|n| n.to_lowercase().contains(&initial_search))
        .map(|n| SharedString::from(n))
        .collect();
    ui.set_game_names(ModelRc::from(Rc::new(VecModel::from(initial_filtered))));

    // Search Callback (to filter the game list as you type)
    let ui_handle_search = ui.as_weak();
    let names_for_search = all_game_names.clone();
    ui.on_search_edited(move |text| {
        if let Some(ui) = ui_handle_search.upgrade() {
            let search_term = text.to_lowercase();
            let filtered: Vec<SharedString> = names_for_search
                .iter()
                .filter(|n| n.to_lowercase().contains(&search_term))
                .map(|n| SharedString::from(n))
                .collect();
            ui.set_game_names(ModelRc::from(Rc::new(VecModel::from(filtered))));
        }
    });

    // Game Selection Callback
    let ui_handle_select = ui.as_weak();
    let games_clone = games.clone();
    let config_select = Arc::clone(&config);
    ui.on_game_selected(move |name| {
        if let Some(ui) = ui_handle_select.upgrade() {
            if let Some(id) = games_clone.get(name.as_str()) {
                ui.set_app_id(SharedString::from(id));

                let mut cfg = config_select.lock().unwrap();

                // Load exe paths for selected game
                let game_cfg = cfg.game_configs.get(id).cloned().unwrap_or_default();
                ui.set_exe1_path(game_cfg.exe1_path.into());
                ui.set_exe2_path(game_cfg.exe2_path.into());

                // Update launch options status
                if cfg.auto_configure {
                    let status = if has_protonhax_configured(id) {
                        "✓ Launch options configured"
                    } else {
                        "Launch options will be configured on launch"
                    };
                    ui.set_launch_options_status(status.into());
                }

                // Save last selected game
                cfg.last_game_name = name.to_string();
                cfg.last_app_id = id.clone();
                let _ = confy::store("protonic", None, &*cfg);
            }
        }
    });

    // Auto-configure toggle callback
    let ui_handle_toggle = ui.as_weak();
    let config_toggle = Arc::clone(&config);
    ui.on_auto_configure_toggled(move |enabled| {
        if let Some(ui) = ui_handle_toggle.upgrade() {
            let mut cfg = config_toggle.lock().unwrap();
            cfg.auto_configure = enabled;
            let _ = confy::store("protonic", None, &*cfg);

            // Update status display
            if enabled && !cfg.last_app_id.is_empty() {
                let status = if has_protonhax_configured(&cfg.last_app_id) {
                    "✓ Launch options configured"
                } else {
                    "Launch options will be configured on launch"
                };
                ui.set_launch_options_status(status.into());
            } else {
                ui.set_launch_options_status(SharedString::new());
            }
        }
    });

    // Browse user's exe 1 Callback
    let ui_handle_browse1 = ui.as_weak();
    let config_browse1 = Arc::clone(&config);
    ui.on_browse_exe1(move || {
        if let Some(ui) = ui_handle_browse1.upgrade() {
            let app_id = ui.get_app_id().to_string();
            if app_id.is_empty() {
                return;
            }

            if let Some(path) = FileDialog::new()
                .add_filter("Executables", &["exe"])
                .add_filter("All Files", &["*"])
                .pick_file()
            {
                let path_str = path.to_string_lossy().to_string();
                ui.set_exe1_path(path_str.clone().into());

                // Save to config
                let mut cfg = config_browse1.lock().unwrap();
                let game_cfg = cfg.game_configs.entry(app_id).or_default();
                game_cfg.exe1_path = path_str;
                let _ = confy::store("protonic", None, &*cfg);
            }
        }
    });

    // Browse user's exe 2 Callback
    let ui_handle_browse2 = ui.as_weak();
    let config_browse2 = Arc::clone(&config);
    ui.on_browse_exe2(move || {
        if let Some(ui) = ui_handle_browse2.upgrade() {
            let app_id = ui.get_app_id().to_string();
            if app_id.is_empty() {
                return;
            }

            if let Some(path) = FileDialog::new()
                .add_filter("Executables", &["exe"])
                .add_filter("All Files", &["*"])
                .pick_file()
            {
                let path_str = path.to_string_lossy().to_string();
                ui.set_exe2_path(path_str.clone().into());

                // Save to config
                let mut cfg = config_browse2.lock().unwrap();
                let game_cfg = cfg.game_configs.entry(app_id).or_default();
                game_cfg.exe2_path = path_str;
                let _ = confy::store("protonic", None, &*cfg);
            }
        }
    });

    // Clear exe 1 Callback
    let ui_handle_clear1 = ui.as_weak();
    let config_clear1 = Arc::clone(&config);
    ui.on_clear_exe1(move || {
        if let Some(ui) = ui_handle_clear1.upgrade() {
            let app_id = ui.get_app_id().to_string();
            ui.set_exe1_path(SharedString::new());

            if !app_id.is_empty() {
                let mut cfg = config_clear1.lock().unwrap();
                if let Some(game_cfg) = cfg.game_configs.get_mut(&app_id) {
                    game_cfg.exe1_path = String::new();
                    let _ = confy::store("protonic", None, &*cfg);
                }
            }
        }
    });

    // Clear exe 2 Callback
    let ui_handle_clear2 = ui.as_weak();
    let config_clear2 = Arc::clone(&config);
    ui.on_clear_exe2(move || {
        if let Some(ui) = ui_handle_clear2.upgrade() {
            let app_id = ui.get_app_id().to_string();
            ui.set_exe2_path(SharedString::new());

            if !app_id.is_empty() {
                let mut cfg = config_clear2.lock().unwrap();
                if let Some(game_cfg) = cfg.game_configs.get_mut(&app_id) {
                    game_cfg.exe2_path = String::new();
                    let _ = confy::store("protonic", None, &*cfg);
                }
            }
        }
    });

    // Launch logic
    let config_launch = Arc::clone(&config);
    ui.on_run_protonhax(move |app_id| {
        let app_id_str = app_id.to_string();

        // Get config values
        let (exe1, exe2, auto_configure) = {
            let cfg = config_launch.lock().unwrap();
            let (e1, e2) = if let Some(game_cfg) = cfg.game_configs.get(&app_id_str) {
                (game_cfg.exe1_path.clone(), game_cfg.exe2_path.clone())
            } else {
                (String::new(), String::new())
            };
            (e1, e2, cfg.auto_configure)
        };

        if exe1.is_empty() {
            println!("No executable selected!");
            return;
        }

        // Auto-configure launch options if enabled
        if auto_configure {
            match configure_launch_options(&app_id_str) {
                Ok(msg) => println!("{}", msg),
                Err(e) => println!("Warning: Could not configure launch options: {}", e),
            }
        }

        // Play launch game audio
        play_audio(AUDIO_LAUNCH_GAME);

        println!("Launching Steam Game {}...", app_id_str);
        let _ = Command::new("steam")
            .arg(format!("steam://run/{}", app_id_str))
            .spawn();

        thread::spawn(move || {
            let device_state = DeviceState::new();
            println!("Waiting for F1...");
            loop {
                let keys = device_state.get_keys();
                if keys.contains(&Keycode::F1) {
                    // Play program launch audio
                    play_audio(AUDIO_LAUNCH_PROGRAM);

                    // Launch exe 1
                    println!("Launching: {}", exe1);
                    let _ = Command::new("protonhax")
                        .arg("run")
                        .arg(&app_id_str)
                        .arg(&exe1)
                        .spawn();

                    // Launch exe 2 (if user set one)
                    if !exe2.is_empty() {
                        println!("Launching: {}", exe2);
                        // Small delay between launches
                        thread::sleep(std::time::Duration::from_millis(500));
                        let _ = Command::new("protonhax")
                            .arg("run")
                            .arg(&app_id_str)
                            .arg(&exe2)
                            .spawn();
                    }
                    break;
                }
                thread::sleep(std::time::Duration::from_millis(100));
            }
        });
    });

    ui.run()?;
    Ok(())
}
