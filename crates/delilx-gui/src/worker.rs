//! Arka plan iş parçacıkları. Arayüz donmasın diye uzun işlemler ayrı iş
//! parçacıklarında çalıştırılır; sonuçlar mesaj kanalıyla iletilir.
//!
//! İş fonksiyonuna paylaşılan durum verilir; böylece işlem sonuçları (örneğin
//! imaj inceleme çıktısı) doğrudan duruma yazılabilir. İş, uzun süren
//! işlemler boyunca durum kilidini tutmamalıdır.

use crate::state::{AppState, SharedState};
use delilx_core::progress::{CancelToken, Progress, ProgressCallback};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

/// Arka planda bir işlem başlatır.
pub fn spawn<F>(state: SharedState, name: &str, job: F)
where
    F: FnOnce(ProgressCallback, CancelToken, Arc<Mutex<AppState>>) -> Result<String, String>
        + Send
        + 'static,
{
    let (tx, rx) = channel::<Progress>();
    let cancel = CancelToken::new();
    {
        let mut s = state.lock().unwrap();
        s.job.running = true;
        s.job.name = name.to_string();
        s.job.progress = None;
        s.job.result = None;
        s.job.cancel = Some(cancel.clone());
        s.job.receiver = Some(rx);
        s.set_status(0, format!("{name} başladı..."));
    }

    let state_clone = Arc::clone(&state);
    let cb: ProgressCallback = Arc::new(move |p: Progress| {
        let _ = tx.send(p);
    });

    std::thread::spawn(move || {
        let result = job(cb, cancel, Arc::clone(&state_clone));
        let mut s = state_clone.lock().unwrap();
        s.job.running = false;
        match result {
            Ok(msg) => {
                s.job.success = true;
                s.job.result = Some(msg.clone());
                s.set_status(1, msg);
            }
            Err(e) => {
                s.job.success = false;
                s.job.result = Some(e.clone());
                s.set_status(3, e);
            }
        }
    });
}
