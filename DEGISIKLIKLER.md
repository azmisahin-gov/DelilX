# Değişiklik Kaydı

Bu dosya DelilX'in sürümleri arasındaki değişiklikleri listeler.

Biçim [Keep a Changelog](https://keepachangelog.com/tr/1.0.0/) temel alınarak
hazırlanmıştır; sürüm numaralandırması [Anlamsal Sürümleme](https://semver.org/lang/tr/)
(Semantic Versioning) kurallarına uyar.

---

## [Yayınlanmadı]

### Eklendi — Yazılımsal Yazma Koruması

- **Yazılımsal write blocker:** Harici donanımsal yazma engelleyici
  kullanmadan, yalnızca yazılım üzerinden USB bellek veya SD kartı yazmaya
  karşı korur (Windows disk salt-okunur özniteliği:
  `IOCTL_DISK_SET_DISK_ATTRIBUTES` + `DISK_ATTRIBUTE_READ_ONLY`).
- **Güvenlik → Yazma Koruması** arayüz ekranı: kullanıcı aygıtı seçer,
  korumayı etkinleştirir/kaldırır ve mevcut durumu görür.
- Koruma **etkinleştirildikten sonra doğrulanır**: salt-okunur özniteliği
  yeniden okunur ve cihaz yazma erişimiyle açılamıyorsa koruma "doğrulandı"
  sayılır. Koruma sağlanamazsa kullanıcı açıkça bilgilendirilir.
- Durum rozetleri: yazmaya kapalı (yeşil), yazılabilir (turuncu), doğrulanamadı
  (kırmızı).
- Donanımsal ve yazılımsal koruma **ayrı** değerlendirilir; yazılımsal korumanın
  donanımsal ile eşdeğer olmadığı arayüzde ve dokümantasyonda belirtilir.
- Cihaz algılama (`Cihazları Algıla`), gerçek koruma durumunu (yazılımsal/
  donanımsal/yok) gösterecek şekilde güncellendi.
- Sistem diski güvenlik nedeniyle korunamaz.
- **CLI:** `koruma durum`, `koruma etkinlestir`, `koruma kaldir` komutları.
- Koruma, `Persist = FALSE` ile ayarlanır (cihaz çıkarılana veya sistem yeniden
  başlatılana kadar etkin); delil cihazı kalıcı olarak değiştirilmez.

### Eklenecek

- NTFS dosya sistemi dizin ağacı çözümleyicisi.
- exFAT dosya sistemi dizin ağacı çözümleyicisi.
- Kullanılan alan ediniminde dosya sistemi düzeyinde boş alan haritalama.

---

## [0.1.0] — 2026-11-17

DelilX'in ilk sürümü. Tamamen Rust ile geliştirilmiş, Windows 10/11 üzerinde
çalışan, Türkçe adli bilişim veri edinim ve inceleme uygulaması.

### Eklendi — Veri Edinimi

- **RAW / DD imaj edinimi:** Sektör sektör okuma, okuma hatası kaydı
  (sıfırla/durdur politikaları), SHA-256 ve SHA-512 doğrulaması, ilerleme
  göstergesi, işlem iptali.
- **E01 (EnCase 6) imaj edinimi:** Gerçek EnCase uyumlu E01 biçimi; zlib
  sıkıştırma, segmentlere ayırma, delil ve uzman bilgileri, MD5 ve SHA-1 hash
  değerleri, blok düzeyinde adler32 sağlamaları.
- **E01 imaj okuma:** Segment dosyalarını çözümleme, rastgele erişimli okuma,
  header2 alanlarını ayrıştırma.
- **Mantıksal edinim:** Mevcut dosyaları alma, klasör yapısını koruma,
  metadata ve hash kaydı, dosya envanteri.
- **Seçmeli edinim:** Kullanıcının seçtiği dosya ve klasörleri alma.
- **Kullanılan alan edinimi:** Yalnızca kullanılan sektörleri alma; silinmiş
  veri kaybı konusunda açık uyarı.

### Eklendi — Kaynak Bellek Koruma

- USB bellek ve SD kart otomatik algılama (Windows).
- Fiziksel disk bilgileri (kapasite, seri numarası, dosya sistemi, sektör boyutu).
- Yazma koruması kontrolü (`IOCTL_DISK_IS_WRITABLE`).
- Donanımsal ve yazılımsal yazma korumasının **ayrı** değerlendirilmesi.
- Kaynak/hedef ayrımı ve aynı aygıt reddi.
- Kaynağa yalnızca okuma erişimi (`GENERIC_READ`); yazma erişimi istenmez.

### Eklendi — Hash ve Doğrulama

- SHA-256, SHA-512, SHA-1, MD5; tek geçişli çoklu hash hesaplama.
- Kaynak/hedef hash karşılaştırması ve doğrulama raporu.
- Hash değerleri ile metadata bütünlüğünün ayrı değerlendirilmesi.
- E01 bütünlük denetimi (blok sağlamaları).

### Eklendi — İmaj İnceleme

- RAW ve E01 imaj açma.
- MBR ve GPT bölüm tablosu çözümleme.
- NTFS, FAT12/16/32, exFAT dosya sistemi tespiti.
- FAT12/16/32 dizin ağacı çözümleme (kısa ad ve uzun dosya adı desteği).
- Dosya hash hesaplama ve dosya çıkarma.

### Eklendi — Metadata

- Dosya adı, yolu, boyutu, oluşturulma/değiştirilme/erişim tarihleri.
- Dosya öznitelikleri (salt okunur, gizli, sistem, arşiv).
- Dosya sistemi ve bölüm bilgileri.
- Korunamayan metadata alanlarının raporda belirtilmesi.

### Eklendi — Raporlama

- Türkçe delil raporu; PDF, JSON ve CSV çıktısı.
- PDF'te DejaVu Sans yazı tipi gömülü (Türkçe karakter desteği).
- Rapor içeriği: işlem/delil numarası, incelemeyi yapan, kaynak cihaz
  bilgileri, edinim yöntemi, zamanlar, dosya sayısı, veri miktarı, hash
  değerleri, doğrulama sonucu, hata ve uyarılar.

### Eklendi — Arayüz

- **Grafik arayüz (eframe/egui):** 12 ekranlı, tamamen Türkçe, modern Windows
  masaüstü tasarımı. Ekranlar: Ana Sayfa, Cihazları Algıla, Veri Edinimi,
  RAW İmaj, E01 İmaj, Mantıksal Edinim, Seçmeli Edinim, Kullanılan Alan,
  İmaj İnceleme, Hash Doğrulama, Delil Raporları, Ayarlar.
- **Komut satırı aracı (clap):** `hakkinda`, `cihazlar`, `raw`, `e01`,
  `mantiksal`, `incele`, `dogrula` komutları.

### Eklendi — Test

- 51 birim testi.
- 11 entegrasyon testi.
- 7 E01 uyumluluk testi (libewf: `ewfinfo`, `ewfverify`, `ewfexport` ile
  bağımsız doğrulama).
- Toplam 69 test.

### Eklendi — Altyapı

- GitHub Actions ile sürekli entegrasyon: biçim denetimi, clippy, testler,
  Windows x64 derlemesi, SHA-256 kontrol dosyası.

### Bilinen Sınırlamalar

- NTFS ve exFAT için dizin ağacı çözümleme henüz uygulanmamıştır; bu dosya
  sistemleri tespit edilir ancak dosya listesi çıkarılamaz.
- Kullanılan alan edinimi bölüm sınırlarına dayalıdır; dosya sistemi düzeyinde
  boş alan haritalama henüz uygulanmamıştır.
- Cihaz algılama yalnızca Windows üzerinde tam çalışır.

---

[Yayınlanmadı]: https://github.com/azmisahin-gov/DelilX/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/azmisahin-gov/DelilX/releases/tag/v0.1.0
