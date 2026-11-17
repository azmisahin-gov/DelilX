# DelilX Kurulum Kılavuzu

Bu kılavuz, DelilX'i Windows 10 / Windows 11 üzerinde kurmayı ve çalıştırmayı
açıklar.

---

## 1. Hazır Sürümü Kurma (Önerilen)

### Gereksinimler

- Windows 10 (64 bit) veya Windows 11
- En az 4 GB RAM (büyük imajlar için daha fazlası önerilir)
- Kaynak cihazı bağlamak için uygun bağlantı (USB, SD kart okuyucu)
- **Adli edinim için donanımsal yazma engelleyici (write blocker) şiddetle önerilir.**

### Adımlar

1. [GitHub Releases](https://github.com/azmisahin-gov/DelilX/releases) sayfasını açın.
2. En son sürümdeki `DelilX-<sürüm>-windows-x64.zip` dosyasını indirin.
3. Arşivi bir klasöre çıkarın.
4. Arşivdeki `SHA256SUMS.txt` dosyasını kullanarak indirdiğiniz arşivin
   bütünlüğünü doğrulayın (aşağıya bakın).
5. `DelilX.exe` dosyasına çift tıklayarak uygulamayı başlatın.

### SHA-256 Doğrulaması (PowerShell)

```powershell
Get-FileHash .\DelilX-0.1.0-windows-x64.zip -Algorithm SHA256
```

Çıkan değeri `SHA256SUMS.txt` içindeki değerle karşılaştırın. Değerler
birebir aynı olmalıdır.

---

## 2. Kaynaktan Derleme

### Gereksinimler

- **Rust** (1.82 veya üzeri). Kurulum: <https://rustup.rs>
- **Visual Studio Build Tools** (C++ derleyicisi ve Windows SDK) — Windows'ta
  Rust'ın `msvc` hedefi için gereklidir.

### Adımlar

```powershell
# Depoyu klonlayın
git clone https://github.com/azmisahin-gov/DelilX.git
cd DelilX

# Windows x64 sürümünü derleyin
cargo build --release

# Grafik arayüzü çalıştırın
.\target\release\DelilX.exe

# Komut satırı aracını çalıştırın
.\target\release\delilx.exe hakkinda
```

### Testleri Çalıştırma

```powershell
cargo test --workspace
```

### E01 Uyumluluk Testleri (İsteğe Bağlı)

E01 uyumluluk testleri, bağımsız doğrulama araçlarına (libewf) ihtiyaç duyar.
Bu araçlar kurulu değilse testler otomatik olarak atlanır.

```powershell
# Windows'ta libewf araçlarını PATH'e ekleyin, ardından:
cargo test --workspace
```

---

## 3. İlk Çalıştırma

Uygulama ilk açıldığında:

1. **Cihazları Algıla** ekranı otomatik olarak açılır ve sistemdeki depolama
   aygıtları listelenir.
2. Kaynak olarak kullanmak istediğiniz aygıtı seçin.
3. **Ayarlar** ekranından varsayılan delil numarası, vaka numarası ve
   incelemeyi yapan bilgilerini girin.
4. Edinim yapmak için ilgili ekrana geçin.

> **Önemli:** Kaynak cihazda doğrulanmış yazma koruması yoksa DelilX edinimi
> başlatmaz ve sizi uyarır. Adli edinim için donanımsal yazma engelleyici
> kullanın.

---

## 4. Kaldırma

DelilX taşınabilir bir uygulamadır; kurulum gerektirmez. Kaldırmak için
uygulama klasörünü silmeniz yeterlidir. Kayıt defterine veya sistem
klasörlerine yazma yapmaz.

---

## 5. Sorun Giderme

| Sorun | Çözüm |
|---|---|
| Cihazlar listelenmiyor | Yönetici olarak çalıştırın. Fiziksel disk erişimi yönetici hakları gerektirir. |
| "Yazma koruması doğrulanamadı" uyarısı | Donanımsal yazma engelleyici bağlayın. Yazılımsal koruma yeterli değildir. |
| E01 dosyası açılmıyor | Dosyanın bütün segmentlerinin (.E01, .E02, ...) aynı klasörde olduğundan emin olun. |
| PDF raporu Türkçe karakter göstermiyor | Bu sürümde DejaVu Sans gömülüdür; sorun olmamalıdır. Eski sürümde güncelleyin. |
| Disk alanı yetersiz | İmajlar kaynak boyutuna yakın yer kaplar. Hedef diskte yeterli alan olduğundan emin olun. |

---

## 6. Destek

Sorun bildirimi için: <https://github.com/azmisahin-gov/DelilX/issues>
