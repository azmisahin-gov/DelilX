//! RAW/DD imaj ekranı.

use crate::screens::{widgets, State};
use crate::theme;
use crate::worker;
use eframe::egui;
use std::path::PathBuf;

/// RAW imaj ekranını çizer.
pub fn draw(ui: &mut egui::Ui, state: &State) {
    ui.label(theme::muted(
        "USB ve SD kartların tam sektör imajını alır. Sektör sektör okuma yapılır, \
         okuma hataları kaydedilir, SHA-256 ve SHA-512 ile doğrulanır.",
    ));
    ui.add_space(10.0);

    widgets::source_target_picker(
        ui,
        state,
        "Kaynak (cihaz/dosya)",
        "Hedef imaj dosyası",
        false,
    );

    let (bps, error_policy, verify, source, target) = {
        let s = state.lock().unwrap();
        (
            s.settings.bytes_per_sector,
            s.raw_error_policy(),
            s.settings.verify_by_default,
            s.source_path.clone(),
            s.target_path.clone(),
        )
    };

    let mut bps_local = bps;
    let mut policy_local = error_policy;
    let mut verify_local = verify;
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Sektör boyutu:").strong());
        for v in [512u32, 4096] {
            ui.radio_value(&mut bps_local, v, format!("{v} bayt"));
        }
        ui.separator();
        ui.label(egui::RichText::new("Okuma hatası:").strong());
        ui.radio_value(&mut policy_local, 0u8, "Sıfırla ve devam et");
        ui.radio_value(&mut policy_local, 1u8, "Durdur");
        ui.separator();
        ui.checkbox(&mut verify_local, "Edinim sonrası doğrula");
    });
    {
        let mut s = state.lock().unwrap();
        s.settings.bytes_per_sector = bps_local;
        s.set_raw_error_policy(policy_local);
        s.settings.verify_by_default = verify_local;
    }

    ui.add_space(10.0);

    let running = state.lock().unwrap().job.running;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !running && !source.is_empty() && !target.is_empty(),
                egui::Button::new("▶  RAW İmajı Oluştur"),
            )
            .clicked()
        {
            start_raw(state);
        }
        if ui
            .add_enabled(!running, egui::Button::new("ℹ  Yöntem Bilgisi"))
            .clicked()
        {
            state.lock().unwrap().set_status(
                0,
                "RAW/DD imaj, kaynağın tam sektör kopyasıdır. Silinmiş veriler ve boş alan korunur.",
            );
        }
    });

    widgets::progress_bar(ui, state);
    widgets::job_result(ui, state);

    ui.add_space(12.0);
    widgets::warning_box(
        ui,
        "Kaynak cihaza yazma yapılmaz. Yazma koruması doğrulanamıyorsa işlem başlatılmaz. \
         Kaynak ve hedef aynı fiziksel aygıtta olamaz.",
    );

    ui.add_space(12.0);
    widgets::log_viewer(ui, state);
}

/// RAW edinimi başlatır.
fn start_raw(state: &State) {
    let (source, target, bps, policy, verify) = {
        let s = state.lock().unwrap();
        (
            s.source_path.clone(),
            s.target_path.clone(),
            s.settings.bytes_per_sector,
            s.raw_error_policy(),
            s.settings.verify_by_default,
        )
    };

    // Kaynak/hedef kontrolü
    if source == target {
        state.lock().unwrap().set_status(
            3,
            "Kaynak ve hedef aynı olamaz. Delili korumak için işlem durduruldu.",
        );
        return;
    }

    {
        let mut s = state.lock().unwrap();
        s.log_info(format!("RAW edinimi başladı: {source} -> {target}"));
    }

    worker::spawn(state.clone(), "RAW İmaj", move |cb, cancel, st| {
        let mut cfg = delilx_core::raw::RawAcquisitionConfig::new(
            PathBuf::from(&source),
            PathBuf::from(&target),
        );
        cfg.bytes_per_sector = bps;
        cfg.error_policy = if policy == 1 {
            delilx_core::raw::ReadErrorPolicy::Abort
        } else {
            delilx_core::raw::ReadErrorPolicy::ZeroFill
        };
        cfg.verify_after = verify;

        let mut src = delilx_core::source::open_source(std::path::Path::new(&source))
            .map_err(|e| format!("Kaynak açılamadı: {e}"))?;
        let result = delilx_core::raw::acquire_raw(&cfg, &mut *src, Some(cb), &cancel)
            .map_err(|e| format!("{e}"))?;

        let mut msg = String::new();
        msg.push_str(&format!("Okunan: {} bayt\n", result.total_bytes));
        msg.push_str(&format!("Sektör: {}\n", result.total_sectors));
        msg.push_str(&format!("Okuma hatası: {}\n", result.error_sector_count));
        for h in &result.source_hashes.values {
            msg.push_str(&format!("{}: {}\n", h.algorithm.name(), h.value));
        }
        match result.verified {
            Some(true) => msg.push_str("Doğrulama: BAŞARILI\n"),
            Some(false) => msg.push_str("Doğrulama: BAŞARISIZ!\n"),
            None => msg.push_str("Doğrulama: yapılmadı\n"),
        }

        // Rapor oluştur
        let mut report = delilx_core::report::EvidenceReport::new();
        {
            let s = st.lock().unwrap();
            report.operation_number = s.report.operation_number.clone();
            report.evidence_number = s.report.evidence_number.clone();
            report.case_number = s.report.case_number.clone();
            report.examiner = s.report.examiner.clone();
            report.description = s.report.description.clone();
        }
        report.source_device.name = source.clone();
        report.source_device.kind = "Dosya/Aygıt".to_string();
        report.source_device.capacity = delilx_core::progress::format_bytes(result.total_bytes);
        report.acquisition_method = "RAW / DD İmaj".to_string();
        report.target = target.clone();
        report.started_at = result.started_at.clone();
        report.finished_at = result.finished_at.clone();
        report.duration = delilx_core::progress::format_duration(result.duration_seconds);
        report.data_bytes = result.total_bytes;
        report.data_size = delilx_core::progress::format_bytes(result.total_bytes);
        report.add_hashes(&result.source_hashes, result.verified);
        report.verification_result = match result.verified {
            Some(true) => "BAŞARILI — kaynak ve hedef hash değerleri eşleşti".to_string(),
            Some(false) => "BAŞARISIZ — hash değerleri uyuşmadı".to_string(),
            None => "Yapılmadı".to_string(),
        };
        report.verification_success = result.verified;
        if result.error_sector_count > 0 {
            report.warnings.push(format!(
                "{} sektör okuma hatası nedeniyle sıfırlarla dolduruldu.",
                result.error_sector_count
            ));
        }
        for e in &result.log.entries {
            match e.level {
                delilx_core::progress::LogLevel::Warning => report.warnings.push(e.message.clone()),
                delilx_core::progress::LogLevel::Error => report.errors.push(e.message.clone()),
                _ => {}
            }
        }
        st.lock().unwrap().last_report = Some(report);

        Ok(msg)
    });
}
