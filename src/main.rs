slint::include_modules!();
use std::process::Command;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;
use slint::{VecModel, SharedString, ModelRc};
use steamlocate::SteamDir;
use device_query::{DeviceQuery, DeviceState, Keycode};
use serde::{Serialize, Deserialize};
use rfd::FileDialog;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct GameConfig {
    exe1_path: String,
    exe2_path: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct AppConfig {
    last_game_name: String,
    last_app_id: String,
    game_configs: HashMap<String, GameConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            last_game_name: String::new(),
            last_app_id: String::new(),
            game_configs: HashMap::new(),
        }
    }
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
        
        // Load exe paths for last selected game if any
        if !cfg.last_app_id.is_empty() {
            if let Some(game_cfg) = cfg.game_configs.get(&cfg.last_app_id) {
                ui.set_exe1_path(game_cfg.exe1_path.clone().into());
                ui.set_exe2_path(game_cfg.exe2_path.clone().into());
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

    // Search Callback (to filter the game list as you type...some ppl have to many games)
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
                
                // Save last selected game
                cfg.last_game_name = name.to_string();
                cfg.last_app_id = id.clone();
                let _ = confy::store("protonic", None, &*cfg);
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
                
                // Now save to config
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
                
                // Now Save to config
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
        
        // Get user-selected exe/s paths from config
        let (exe1, exe2) = {
            let cfg = config_launch.lock().unwrap();
            if let Some(game_cfg) = cfg.game_configs.get(&app_id_str) {
                (game_cfg.exe1_path.clone(), game_cfg.exe2_path.clone())
            } else {
                (String::new(), String::new())
            }
        };

        if exe1.is_empty() {
            println!("No executable selected!");
            return;
        }

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
                    // Launch exe 1
                    println!("Launching: {}", exe1);
                    let _ = Command::new("protonhax")
                        .arg("run")
                        .arg(&app_id_str)
                        .arg(&exe1)
                        .spawn();

                    // Launch exe 2 (if user bothered to set any)
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
