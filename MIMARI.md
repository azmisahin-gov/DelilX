# DelilX Mimari Dokümantasyonu

Bu doküman DelilX'in teknik mimarisini, kullanılan kütüphaneleri ve tasarım
kararlarını açıklar.

---

## 1. Genel Mimari

DelilX üç ayrı Rust crate'inden oluşan bir çalışma alanı (workspace) olarak
yapılandırılmıştır:

```
DelilX/
├── Cargo.toml                 # Çalışma alanı tanımı
├── assets/                    # Gömülü yazı tipleri (PDF için)
│   ├── DejaVuSans.ttf
│   └── DejaVuSans-Bold.ttf
└── crates/
    ├── delilx-core/           # Çekirdek kütüphane (bütün mantık)
    ├── delilx-cli/            # Komut satırı aracı
    └── delilx-gui/            # Grafik arayüz (eframe/egui)
```

Bu ayrım, iş mantığını arayüzden bağımsız tutar: aynı çekirdek kütüphane hem
grafik arayüz hem de komut satırı tarafından kullanılır ve test edilir.

---

## 2. Çekirdek Kütüphane (delilx-core)

### 2.1 Modüller

| Modül | Sorumluluk |
|---|---|
| `error` | Türkçe hata türleri ve mesajları |
| `hash` | SHA-256/512, SHA-1, MD5; tek geçişli çoklu hash |
| `progress` | İlerleme bildirimi, iptal belirteci, işlem kaydı |
| `timeutil` | Zaman biçimlendirme, Windows FILETIME dönüşümü |
| `metadata` | Dosya metadata toplama, dosya envanteri |
| `device` | Cihaz algılama, yazma koruması kontrolü, kaynak/hedef doğrulama |
| `source` | Kaynak okuma soyutlaması (dosya / fiziksel cihaz) |
| `raw` | RAW/DD imaj edinim motoru |
| `e01` | E01 (EWF1) yazıcı ve okuyucu |
| `logical` | Mantıksal ve seçmeli edinim |
| `usedspace` | Kullanılan alan edinimi |
| `fs` | Bölüm tablosu (MBR/GPT) çözümleme, dosya sistemi tespiti |
| `fs_fat` | FAT12/16/32 dizin çözümleyici |
| `inspect` | İmaj inceleme (RAW + E01) |
| `verify` | Hash doğrulama, bütünlük denetimi |
| `report` | Türkçe rapor veri yapısı (PDF/JSON/CSV) |
| `report_pdf` | PDF üretimi (DejaVu Sans gömülü) |
| `acquisition` | Edinim yöntemi tanımları, ön kontroller |
| `writeblock` | Yazılımsal yazma koruması (write blocker) |

### 2.2 Katmanlar

```
┌─────────────────────────────────────────────┐
│  Arayüz (delilx-gui / delilx-cli)           │
├─────────────────────────────────────────────┤
│  Edinim motorları                            │
│  raw · e01 · logical · usedspace             │
├─────────────────────────────────────────────┤
│  İnceleme ve doğrulama                       │
│  inspect · fs · fs_fat · verify              │
├─────────────────────────────────────────────┤
│  Altyapı                                     │
│  source · device · hash · metadata · progress│
├─────────────────────────────────────────────┤
│  Platform (windows-sys / std)                │
└─────────────────────────────────────────────┘
```

---

## 3. Kaynak Okuma Soyutlaması

`SourceReader` trait'i, kaynak erişimini soyutlar:

```rust
pub trait SourceReader: Send {
    fn total_bytes(&self) -> Option<u64>;
    fn bytes_per_sector(&self) -> u32;
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize>;
    fn description(&self) -> String;
}
```

İki uygulaması vardır:

- **`FileSource`** — normal dosyalar (imaj dosyaları, test verileri).
- **`DeviceSource`** — Windows fiziksel cihazlar (`\\.\PhysicalDriveN`).

`DeviceSource` cihazı **yalnızca `GENERIC_READ`** erişimiyle ve
`FILE_FLAG_NO_BUFFERING` ile açar. Yazma erişimi (`GENERIC_WRITE`) hiçbir
zaman istenmez. Bu, kaynak delili korumanın temelidir.

---

## 4. E01 (EWF1) Biçimi

DelilX gerçek EnCase 6 uyumlu E01 dosyaları üretir ve okur. Biçim, libewf
(Joachim Metz) kaynak tanımlarına ve referans dosyalarla yapılan bayt düzeyi
doğrulamaya dayanır.

### 4.1 Dosya Yapısı

```
Dosya başlığı (13 bayt)
  "EVF\x09\x0d\x0a\xff\x00" | alanlar(1) | segment_no(2 LE) | son(2)
Bölüm tanımlayıcı (76 bayt)
  tür(16) | sonraki_ofset(8 LE) | boyut(8 LE) | dolgu(40) | adler32(4 LE)
Bölüm gövdesi (boyut - 76 bayt)
```

- `sonraki_ofset` dosyadaki **mutlak** bir sonraki bölüm ofsetidir.
- Bölüm tanımlayıcı sağlaması, ilk 72 baytın adler32 değeridir.

### 4.2 Segment Düzeni

- **Segment 1:** `header2`, `header2`, `header`, `volume`, `sectors`, `table`, `table2`
- **Sonraki segmentler:** `data`, `sectors`, `table`, `table2`
- **Ara segment sonu:** `next`
- **Son segment sonu:** `digest`, `hash`, `done`

### 4.3 Blok (Chunk) Düzeni

- Her blok `sectors_per_chunk` sektör içerir (varsayılan 64 → 32 KiB).
- Blok tablosundaki her giriş 4 bayttır: ofset (bit 0-30) + sıkıştırma bayrağı (bit 31).
- Ofsetler, `sectors` bölümünün başlangıcına (base offset) göredir ve **kümülatiftir**.
- Sıkıştırılmış bloklar: zlib akışı.
- Sıkıştırılmamış bloklar: ham veri + adler32 (4 bayt LE).

### 4.4 Bölüm Gövdeleri

**volume / data (1052 bayt):** medya türü, blok sayısı, sektör/blok,
bayt/sektör, sektör sayısı, medya bayrakları, sıkıştırma seviyesi, hata
granülaritesi + adler32.

**table / table2:** 4 bayt giriş sayısı + 4 dolgu + 8 bayt base offset + 4
dolgu + adler32(başlık) + girişler + adler32(girişler).

**digest (80 bayt):** MD5(16) + SHA-1(20) + dolgu(40) + adler32.

**hash (36 bayt):** MD5(16) + dolgu(16) + adler32.

**header2:** UTF-16LE (BOM'lu) metin, zlib ile sıkıştırılmış. EnCase 6
başlık şablonunu izler.

### 4.5 Bağımsız Doğrulama

Üretilen E01 imajları libewf araçlarıyla doğrulanır:

- `ewfinfo` — imajı EnCase 6 olarak tanımalı.
- `ewfverify` — `SUCCESS` döndürmeli.
- `ewfexport` — veriyi birebir çıkarabilmeli.

Ayrıca DelilX okuyucusu, libewf'in ürettiği referans E01 dosyalarını
okuyabilir (ters yön uyumluluk).

---

## 5. Cihaz Algılama ve Yazma Koruması

### 5.1 Cihaz Algılama

`GetLogicalDrives` ile sürücü harfleri taranır, `GetDriveTypeW` ile tür
belirlenir. Her sürücü için:

- `GetDiskFreeSpaceExW` — kapasite.
- `GetVolumeInformationW` — birim etiketi, dosya sistemi, seri numarası.
- `IOCTL_STORAGE_GET_DEVICE_NUMBER` — fiziksel sürücü numarası.
- `IOCTL_DISK_GET_DRIVE_GEOMETRY_EX` — sektör boyutu.

### 5.2 Yazma Koruması

`IOCTL_DISK_IS_WRITABLE` ile denetlenir:

- Sorgu başarısız ve hata kodu `ERROR_WRITE_PROTECT` (19) ise → **donanımsal**.
- Sorgu başarılı (yazılabilir) ise → **koruma yok**.
- Diğer hatalar → **doğrulanamadı**.

Yazılımsal salt okunur koruma, donanımsal yazma engelleyiciyle **eşdeğer
gösterilmez** ve raporda ayrı belirtilir.

### 5.3 Kaynak/Hedef Doğrulama

`validate_source_target`:

1. Kaynak ve hedef aynı fiziksel sürücüde ise → hata.
2. Hedef, kaynağın birim harfi altında ise → hata.
3. Kaynakta doğrulanmış yazma koruması yoksa → hata.

Bu kontrollerden biri başarısız olursa edinim başlatılmaz.

---

## 6. Dosya Sistemi Çözümleme

### 6.1 Bölüm Tablosu

- **MBR:** 0x1BE'den itibaren 4 × 16 bayt giriş.
- **GPT:** LBA 1'deki başlık, LBA 2'den bölüm girişleri.

### 6.2 Dosya Sistemi Tespiti

Bölümün başındaki imzalar:

- NTFS: `"NTFS"` @ ofset 3.
- exFAT: `"EXFAT"` @ ofset 3.
- FAT12/16/32: `"FATxx"` @ ofset 54 veya 82.

### 6.3 FAT Çözümleyici

- Önyükleme kesimi (boot sector) ayrıştırılır.
- FAT türü, önyükleme kesimindeki fs_type metninden okunur (güvenilir yöntem).
- Kök dizin (FAT32'de küme zinciri, FAT12/16'da sabit bölge) dolaşılır.
- Kısa ad (8.3) ve uzun dosya adı (LFN) girdileri çözümlenir.
- Küme zinciri FAT tablosundan izlenir.

---

## 7. Raporlama

### 7.1 Veri Modeli

`EvidenceReport` yapısı, bütün rapor alanlarını Türkçe etiketlerle taşır.

### 7.2 Biçimler

- **JSON:** `serde_json` ile serileştirilir.
- **CSV:** alan başına satır; `Alan,Değer` başlığı.
- **PDF:** `printpdf` ile üretilir. **DejaVu Sans** yazı tipi gömülüdür;
  böylece Türkçe karakterler (ç, ğ, ı, İ, ö, ş, ü) doğru görüntülenir.

---

## 8. Eşzamanlılık ve İptal

- Uzun süren işlemler ayrı iş parçacıklarında çalışır.
- İlerleme, `ProgressCallback` ile bildirilir (mesaj kanalıyla arayüze aktarılır).
- `CancelToken` atomik bayrak ile iptali destekler; işlem döngüsü her adımda
  `cancel.check()` çağırır.
- İptal edilen işlem `DelilXError::Cancelled` döndürür.

---

## 9. Bağımlılıklar

| Crate | Amaç |
|---|---|
| `sha2`, `sha1`, `md-5` | Hash algoritmaları |
| `flate2` | zlib/deflate sıkıştırma (E01) |
| `serde`, `serde_json` | Serileştirme |
| `chrono` | Tarih/saat |
| `walkdir` | Dizin dolaşma |
| `printpdf` | PDF üretimi |
| `eframe`, `egui` | Grafik arayüz |
| `rfd` | Dosya seçme diyalogları |
| `clap` | Komut satırı ayrıştırma |
| `windows-sys` | Windows API erişimi |

Bütün bağımlılıklar açık kaynak ve yaygın kullanılan kütüphanelerdir.

---

## 10. Tasarım Kararları

**Neden saf Rust?** Tek bir dil ile hem performans hem güvenlik sağlanır;
harici çalışma zamanı bağımlılığı yoktur.

**Neden `eframe`/`egui`?** Saf Rust, native, çapraz platform; GPU hızlandırmalı;
Türkçe karakter desteği için yazı tipi gömülebilir.

**Neden libewf uyumlu E01?** Gerçek uyumluluk, bağımsız araçlarla
doğrulanabilirlik ve adli bilişim camiasında kabul görmüş biçim sağlar.

**Neden hash ve metadata ayrı?** Hash eşleşmesi veri bütünlüğünü gösterir;
metadata bütünlüğü (zaman damgaları, öznitelikler) ayrı bir bütünlük boyutudur
ve ayrı değerlendirilmelidir.

---

## 11. Yazılımsal Yazma Koruması

`writeblock` modülü, Windows disk salt-okunur özniteliğini kullanarak harici
donanım gerektirmeden yazma koruması sağlar.

### 11.1 Kullanılan API

| İşlem | IOCTL |
|---|---|
| Salt-okunur özniteliğini oku | `IOCTL_DISK_GET_DISK_ATTRIBUTES` |
| Salt-okunur özniteliğini ayarla | `IOCTL_DISK_SET_DISK_ATTRIBUTES` |
| Yazma engelini denetle | `IOCTL_DISK_IS_WRITABLE` |

`SET_DISK_ATTRIBUTES` yapısı `DISK_ATTRIBUTE_READ_ONLY` bayrağı ve
`AttributesMask` ile ayarlanır. `Persist = FALSE` seçilir; koruma yalnızca
çalışma zamanında geçerlidir ve delil cihazını kalıcı olarak değiştirmez.

### 11.2 Durum Sınıflandırması

```
salt-okunur özniteliği = true                    -> Yazılımsal (DelilX)
salt-okunur = false  VE  yazılabilir = false     -> Donanımsal
salt-okunur = false  VE  yazılabilir = true      -> Yok (yazılabilir)
sorgu başarısız                                  -> Doğrulanamadı
```

Bu ayrım, yazılımsal korumanın donanımsal ile karıştırılmamasını garanti eder.

### 11.3 Doğrulama

Koruma etkinleştirildikten sonra:

1. Salt-okunur özniteliği yeniden okunur.
2. Cihaz `GENERIC_READ | GENERIC_WRITE` ile açılmaya çalışılır; başarısız
   olması beklenir.

Her iki kontrol de geçerse koruma "doğrulandı" sayılır. Aksi hâlde kullanıcı
açıkça uyarılır.

### 11.4 Güvenlik Sınırları

- Sistem diski korunamaz (`is_system_disk` denetimi).
- Yönetici hakları gerekir.
- Yazılımsal koruma, donanımsal engelleyiciyle eşdeğer **değildir** ve bu fark
  arayüzde, CLI çıktısında ve dokümantasyonda açıkça belirtilir.
