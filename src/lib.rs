use std::{thread::JoinHandle, sync::{Arc, Mutex}, collections::VecDeque};
use input::{handle_input, refresh};
use lazy_static::lazy_static;
use terminal_size::{terminal_size, Width, Height};

lazy_static! {
    static ref KONSOLE: Mutex<Konsole> = Mutex::new(Konsole::new());
    static ref SETTINGS: Mutex<Settings> = Mutex::new(Settings::new());
}

mod input;
mod getch;
pub mod print;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as system;
#[cfg(not(windows))]
mod unix;
#[cfg(not(windows))]
use unix as system;

struct Konsole {
    initialized: bool,
    active: bool,
    running: bool,
    input_thread: Option<JoinHandle<()>>,
    queued_inputs: Vec<String>,
    input: String,
    temp_input: Option<String>,
    cursor: usize,
    history: VecDeque<String>,
    history_index: usize,
    tab_repeat: usize,
    cursor_before_tab: usize
}

impl Konsole {
    fn new() -> Self {
        Self {
            active: false,
            initialized: false,
            running: false,
            input_thread: None,
            queued_inputs: vec![],
            input: String::new(),
            temp_input: None,
            cursor: 0,
            history: VecDeque::new(),
            history_index: 0,
            tab_repeat: 0,
            cursor_before_tab: 0
        }
    }
}

pub fn activate() {
    let mut konsole = KONSOLE.lock().unwrap();
    if konsole.active { return; }
    if !konsole.initialized {
        enable_ansi_support::enable_ansi_support().expect("could not enable ansi support");
        konsole.initialized = true;
    }
    konsole.active = true;
    konsole.running = true;
    konsole.input_thread = Some(std::thread::spawn(handle_input));
    // sdropping os that refresh can be called
    drop(konsole);
    refresh();
}

pub fn deactivate(exit_prompt: Option<&str>) {
    let mut konsole = KONSOLE.lock().unwrap();
    // stopped or already in process of stopping?
    if !konsole.active || !konsole.running { return; }
    // signal stop
    konsole.running = false;
    let handle = konsole.input_thread.take().unwrap();
    // free the ref to allow the input thread to take ref
    drop(konsole);
    //// clear the input row
    //clear_input_row(true); 
    // display optional prompt to press any key to exit
    if let Some(p) = exit_prompt { crate::println!("{p}"); }
    // wait for thread to terminate
    handle.join().expect("error terminating konsole input thread");
    // take reference again to complete termination
    let mut konsole = KONSOLE.lock().unwrap();
    konsole.active = false;
}

pub fn is_active() -> bool {
    KONSOLE.lock().unwrap().active
}

pub fn is_locked() -> bool {
    KONSOLE.try_lock().is_err()
}

pub fn queued_inputs() -> Vec<String> {
    std::mem::take(&mut KONSOLE.lock().unwrap().queued_inputs)
}

pub fn size() -> (usize, usize) {
    if let Some((Width(w), Height(h))) = terminal_size() {
        (w as usize, h as usize)
    } else {
        panic!("\rUnable to get terminal size. Please use different terminal");
    }
}

pub fn clear_input() {
    let mut konsole = KONSOLE.lock().unwrap();
    konsole.input.clear();
    konsole.cursor = 0;
}

pub fn clear_history() {
    let mut konsole = KONSOLE.lock().unwrap();
    konsole.history.clear();
    konsole.history_index = 0;
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub history_enabled: bool,
    pub history_limit: usize,
    pub prompt: String,
    pub tab_complete: Arc<fn(query: TabQuery) -> Option<TabResult>>
}

impl Settings {
    fn new() -> Self {
        Self {
            history_enabled: false,
            history_limit: 256,
            prompt: ">".to_string(),
            tab_complete: Arc::new(tab_spaces)
        }
    }
}

pub fn edit_settings(edit: impl Fn(&mut Settings)) {
    let mut settings = SETTINGS.lock().unwrap();
    edit(&mut settings);
    if KONSOLE.lock().unwrap().active {
        refresh();
    }
}

pub fn tab_spaces(query: TabQuery) -> Option<TabResult> {
    Some(TabResult { output: query.input + "    ", cursor_movement: 4 })
}

pub fn tab_nothing(query: TabQuery) -> Option<TabResult> {
    None
}

pub struct TabQuery {
    pub input: String,
    pub cursor_position: usize,
    pub cursor_before: usize,
    pub tab_repeat: usize
}

pub struct TabResult {
    pub output: String,
    pub cursor_movement: isize
}