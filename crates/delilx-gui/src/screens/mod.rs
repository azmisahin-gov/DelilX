//! Arayüz ekranları.

pub mod acquisition;
pub mod devices;
pub mod e01;
pub mod home;
pub mod inspect;
pub mod logical;
pub mod raw;
pub mod reports;
pub mod settings;
pub mod usedspace;
pub mod verify;
pub mod widgets;
pub mod writeprotect;

use crate::state::AppState;
use std::sync::{Arc, Mutex};

/// Paylaşılan durum türü.
pub type State = Arc<Mutex<AppState>>;
