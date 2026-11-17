//! E01 (EnCase) imaj ekranı.

use crate::screens::{widgets, State};
use crate::theme;
use crate::worker;
use eframe::egui;
use std::path::PathBuf;

/// E01 imaj ekranını çizer.
pub fn draw(ui: &mut egui::Ui, state: &State) {
    ui.label(theme::muted(
        "Gerçek EnCase uyumlu E01 imajı oluşturur. Sıkıştırma, parçalara ayırma, \
         delil ve uzman bilgileri, MD5/SHA-1 hash doğrulaması desteklenir.",
    ));
    ui.add_space(10.0);

    widgets::source_target_picker(
        ui,
        state,
        "Kaynak (cihaz/dosya)",
        "Hedef E01 dosyası",
        false,
    );

    ui.add_space(8.0);
    ui.label(theme::subheading("Delil ve Uzman Bilgileri"));
    {
        let mut s = state.lock().unwrap();
        egui::Grid::new("e01_form")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Delil numarası");
                ui.add(egui::TextEdit::singleline(&mut s.e01.evidence_number).desired_width(360.0));
                ui.end_row();
                ui.label("Vaka numarası");
                ui.add(egui::TextEdit::singleline(&mut s.e01.case_number).desired_width(360.0));
                ui.end_row();
                ui.label("İncelemeyi yapan");
                ui.add(egui::TextEdit::singleline(&mut s.e01.examiner).desired_width(360.0));
                ui.end_row();
                ui.label("Açıklama");
                ui.add(egui::TextEdit::singleline(&mut s.e01.description).desired_width(360.0));
                ui.end_row();
                ui.label("Notlar");
                ui.add(egui::TextEdit::singleline(&mut s.e01.notes).desired_width(360.0));
                ui.end_row();
            });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Sıkıştırma:").strong());
            ui.radio_value(&mut s.e01.compression, 0u8, "Yok");
            ui.radio_value(&mut s.e01.compression, 1u8, "Hızlı");
            ui.radio_value(&mut s.e01.compression, 2u8, "En iyi");
            ui.separator();
            ui.label(egui::RichText::new("Segment boyutu (MiB):").strong());
            ui.add(egui::DragValue::new(&mut s.e01.segment_mib).range(1..=100_000));
            ui.separator();
            ui.checkbox(&mut s.e01.removable, "Çıkarılabilir aygıt");
        });
    }

    ui.add_space(10.0);
    let running = state.lock().unwrap().job.running;
    let (source, target) = {
        let s = state.lock().unwrap();
        (s.source_path.clone(), s.target_path.clone())
    };

    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !running && !source.is_empty() && !target.is_empty(),
                egui::Button::new("▶  E01 İmajı Oluştur"),
            )
            .clicked()
        {
            start_e01(state);
        }
    });

    widgets::progress_bar(ui, state);
    widgets::job_result(ui, state);

    ui.add_space(12.0);
    widgets::warning_box(
        ui,
        "Oluşturulan E01 imajları bağımsız doğrulama araçlarıyla (ewfinfo, ewfverify) \
         test edilir. Format EnCase 6 uyumludur.",
    );

    ui.add_space(12.0);
    widgets::log_viewer(ui, state);
}

/// E01 edinimi başlatır.
fn start_e01(state: &State) {
    let (source, target, form, bps) = {
        let s = state.lock().unwrap();
        (
            s.source_path.clone(),
            s.target_path.clone(),
            s.e01.clone(),
            s.settings.bytes_per_sector,
        )
    };

    if source == target {
        state.lock().unwrap().set_status(
            3,
            "Kaynak ve hedef aynı olamaz. Delili korumak için işlem durduruldu.",
        );
        return;
    }

    {
        let mut s = state.lock().unwrap();
        s.log_info(format!("E01 edinimi başladı: {source} -> {target}"));
    }

    worker::spawn(state.clone(), "E01 İmaj", move |cb, cancel, st| {
        let mut cfg = delilx_core::e01::EwfWriterConfig::new(PathBuf::from(&target));
        cfg.case_info = delilx_core::e01::writer::EwfCaseInfo {
            evidence_number: form.evidence_number.clone(),
            case_number: form.case_number.clone(),
            description: form.description.clone(),
            examiner_name: form.examiner.clone(),
            notes: form.notes.clone(),
            ..Default::default()
        };
        cfg.bytes_per_sector = bps;
        cfg.compression = match form.compression {
            0 => delilx_core::e01::writer::CompressionMode::None,
            1 => delilx_core::e01::writer::CompressionMode::Fast,
            _ => delilx_core::e01::writer::CompressionMode::Best,
        };
        cfg.media_type = if form.removable {
            delilx_core::e01::writer::MediaType::Removable
        } else {
            delilx_core::e01::writer::MediaType::Fixed
        };
        cfg.segment_size_bytes = form.segment_mib * 1024 * 1024;

        let mut src = delilx_core::source::open_source(std::path::Path::new(&source))
            .map_err(|e| format!("Kaynak açılamadı: {e}"))?;
        let result = delilx_core::e01::EwfWriter::acquire(cfg, &mut *src, Some(cb), &cancel)
            .map_err(|e| format!("{e}"))?;

        let mut msg = String::new();
        msg.push_str(&format!(
            "Segment dosyaları: {}\n",
            result.segment_files.join(", ")
        ));
        msg.push_str(&format!(
            "Toplam: {} ({} bayt)\n",
            delilx_core::progress::format_bytes(result.total_bytes),
            result.total_bytes
        ));
        msg.push_str(&format!("Blok sayısı: {}\n", result.total_chunks));
        msg.push_str(&format!("MD5 : {}\n", result.md5));
        msg.push_str(&format!("SHA1: {}\n", result.sha1));
        for h in &result.extra_hashes.values {
            msg.push_str(&format!("{}: {}\n", h.algorithm.name(), h.value));
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
        report.source_device.kind = if form.removable {
            "Çıkarılabilir Aygıt (USB/SD)".to_string()
        } else {
            "Sabit Disk".to_string()
        };
        report.source_device.capacity = delilx_core::progress::format_bytes(result.total_bytes);
        report.acquisition_method = "E01 (EnCase) İmaj".to_string();
        report.target = result.segment_files.join(", ");
        report.started_at = result.started_at.clone();
        report.finished_at = result.finished_at.clone();
        report.duration = delilx_core::progress::format_duration(result.duration_seconds);
        report.data_bytes = result.total_bytes;
        report.data_size = delilx_core::progress::format_bytes(result.total_bytes);
        let mut hs = result.extra_hashes.clone();
        hs.insert(delilx_core::hash::HashAlgorithm::Md5, result.md5.clone());
        hs.insert(delilx_core::hash::HashAlgorithm::Sha1, result.sha1.clone());
        report.add_hashes(&hs, None);
        report.verification_result =
            "E01 içine MD5 ve SHA-1 yazıldı; bağımsız araçla doğrulanabilir".to_string();
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
