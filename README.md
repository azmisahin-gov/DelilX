# DelilX

> **Dijital Delili Koru, Doğrula ve İncele.**

DelilX, tamamen **Rust** ile geliştirilmiş, **Windows 10 ve Windows 11** üzerinde
çalışan profesyonel bir **adli bilişim veri edinim ve inceleme** uygulamasıdır.
Arayüzü, dokümantasyonu ve raporları tamamen **Türkçedir**.

Python, Electron, Node.js veya benzeri bir çalışma zamanı **kullanılmaz**.
Uygulama internet bağlantısı olmadan çalışır.

---

## Özellikler

### Veri Edinim Yöntemleri

| Yöntem | Açıklama |
|---|---|
| **RAW / DD** | USB ve SD kartların tam sektör imajı. Sektör sektör okuma, okuma hatası kaydı, SHA-256 / SHA-512 doğrulaması. |
| **E01 (EnCase)** | Gerçek EnCase uyumlu E01 imajı. Sıkıştırma, parçalara ayırma, delil ve uzman bilgileri, MD5 / SHA-1 hash doğrulaması. |
| **Mantıksal Edinim** | Yalnızca mevcut dosyaları alır, klasör yapısını korur, metadata ve hash kaydeder. |
| **Seçmeli Edinim** | Kullanıcının seçtiği dosya ve klasörleri alır, dosya envanteri oluşturur. |
| **Kullanılan Alan** | Yalnızca kullanılan sektörleri alır. Boş alan dışarıda bırakılır. |

### Kaynak Bellek Koruma (En Önemli Güvenlik Gereksinimi)

- USB bellekler ve SD kartlar otomatik algılanır.
- Fiziksel disk bilgileri (kapasite, seri numarası, dosya sistemi) gösterilir.
- Kaynak ve hedef diskler birbirinden ayrılır; aynı aygıt seçilemez.
- Kaynak cihaza **hiçbir koşulda yazma** yapılmaz; yalnızca okuma erişimi kullanılır.
- Yazma koruması kontrol edilir; donanımsal yazma engelleyiciler desteklenir.
- **Yazılımsal salt okunur koruma, donanımsal yazma engelleyiciyle eşdeğer gösterilmez.**
- Koruma doğrulanamıyorsa kullanıcı uyarılır ve edinim başlatılmaz.
- Kaynak hiçbir koşulda otomatik biçimlendirilmez veya onarılmaz.

### Hash ve Veri Bütünlüğü

- SHA-256 ve SHA-512.
- Kaynak ve hedef karşılaştırması, dosya boyutu kontrolü.
- Metadata envanteri, edinim başlangıç/bitiş zamanları, işlem kayıtları.
- **Hash değerleri ile metadata bütünlüğü ayrı değerlendirilir.**

### İmaj İnceleme

- RAW ve E01 imaj açma.
- Bölümleri listeleme, dosya sistemlerini tespit etme.
- FAT12/16/32 dosya sistemlerinde dosya listeleme ve metadata görüntüleme.
- Dosya hash hesaplama, dosyaları ayrı bir klasöre çıkarma.
- **Orijinal imaj dosyası hiçbir koşulda değiştirilmez.**

### Raporlama

Türkçe delil raporu; **PDF**, **JSON** ve **CSV** biçimlerinde. Rapor; işlem
numarası, delil numarası, incelemeyi yapan kişi, kaynak cihaz bilgileri, edinim
yöntemi, başlangıç/bitiş zamanı, dosya sayısı, veri miktarı, hash değerleri,
doğrulama sonucu ile hata ve uyarıları içerir.

---

## Programın Türkçe Arayüzü

Ana ekranda şu bölümler bulunur:

- Ana Sayfa
- Cihazları Algıla
- Veri Edinimi
  - RAW İmaj
  - E01 İmaj
  - Mantıksal Edinim
  - Seçmeli Edinim
  - Kullanılan Alan
- İmaj İnceleme
- Hash Doğrulama
- Delil Raporları
- Ayarlar

---

## E01 Uyumluluğu

DelilX'in ürettiği E01 imajları **EnCase 6** biçimindedir ve **bağımsız
doğrulama araçlarıyla** (libewf: `ewfinfo`, `ewfverify`, `ewfexport`) test
edilir. E01 biçimi taklit edilmez; gerçek uyumluluk sağlanır.

```bash
# Bağımsız doğrulama
ewfinfo   cikti.E01     # EnCase 6 olarak tanınmalı
ewfverify cikti.E01     # "SUCCESS" döndürmeli
```

Ayrıntı için [TESTLER.md](TESTLER.md) dosyasına bakın.

---

## Kurulum ve Derleme

Ayrıntılı adımlar için [KURULUM.md](KURULUM.md) dosyasına bakın.

### Hızlı Başlangıç (Windows x64)

1. [GitHub Releases](https://github.com/azmisahin-gov/DelilX/releases) sayfasından
   en son sürümü indirin.
2. `DelilX-<sürüm>-windows-x64.zip` arşivini açın.
3. `DelilX.exe` dosyasını çalıştırın.
4. `SHA256SUMS.txt` dosyası ile indirdiğiniz arşivin bütünlüğünü doğrulayın.

### Kaynaktan Derleme

```powershell
git clone https://github.com/azmisahin-gov/DelilX.git
cd DelilX
cargo build --release
```

---

## Kullanım

Ayrıntılı kullanım için [KULLANIM_KILAVUZU.md](KULLANIM_KILAVUZU.md) dosyasına bakın.

### Grafik Arayüz

```powershell
cargo run --release -p delilx-gui
```

### Komut Satırı

```powershell
# Hakkında
delilx-cli hakkinda

# Cihazları listele
delilx-cli cihazlar

# RAW imaj al
delilx-cli raw --kaynak "\\.\PhysicalDrive2" --hedef "D:\delil\usb.dd"

# E01 imaj al
delilx-cli e01 --kaynak "\\.\PhysicalDrive2" --hedef "D:\delil\usb.E01" `
           --delil-no "DELIL-1" --uzman "Ad Soyad"

# İmajı incele
delilx-cli incele --imaj "D:\delil\usb.E01"

# Hash doğrula
delilx-cli dogrula --imaj "D:\delil\usb.dd" --sha256 "<beklenen-sha256>"
```

---

## Dokümantasyon

| Dosya | İçerik |
|---|---|
| [KURULUM.md](KURULUM.md) | Kurulum ve derleme adımları |
| [KULLANIM_KILAVUZU.md](KULLANIM_KILAVUZU.md) | Adım adım kullanım kılavuzu |
| [MIMARI.md](MIMARI.md) | Teknik mimari ve tasarım kararları |
| [GUVENLIK.md](GUVENLIK.md) | Güvenlik ilkeleri ve kaynak bellek koruma |
| [TESTLER.md](TESTLER.md) | Test stratejisi ve E01 uyumluluk testleri |
| [DEGISIKLIKLER.md](DEGISIKLIKLER.md) | Sürüm değişiklik kaydı |

---

## Teknoloji

- **Dil:** Saf Rust (2021 sürümü)
- **Arayüz:** `eframe` / `egui` (native Windows masaüstü)
- **Sıkıştırma:** `flate2` (zlib/deflate)
- **Hash:** `sha2`, `sha1`, `md-5`
- **Rapor:** `printpdf` (DejaVu Sans yazı tipi gömülü)
- **Windows API:** `windows-sys`

Bütün bağımlılıklar açık kaynak ve güvenilirdir.

---

## Test

```bash
cargo test --workspace
```

Ayrıntı için [TESTLER.md](TESTLER.md) dosyasına bakın.

---

## Katkı

Bu proje Türkçe hazırlanmaktadır. Kod içerisinde gerekli teknik isimler
İngilizce olabilir; ancak kullanıcıya gösterilen hiçbir açıklama gereksiz yere
İngilizce olmamalıdır.

---

## Lisans

MIT Lisansı. Ayrıntı için [LICENSE](LICENSE) dosyasına bakın.

---

**DelilX** — Dijital Delili Koru, Doğrula ve İncele.
