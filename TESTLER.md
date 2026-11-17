# DelilX Test Dokümantasyonu

Bu doküman DelilX'in test stratejisini, kapsamını ve E01 uyumluluk
doğrulamasını açıklar.

---

## 1. Test Stratejisi

DelilX üç katmanlı test yaklaşımı kullanır:

1. **Birim testleri** — Her modülün kendi içinde test edilmesi (`#[cfg(test)]`).
2. **Entegrasyon testleri** — Uçtan uca veri akışlarının test edilmesi.
3. **E01 uyumluluk testleri** — Bağımsız araçlarla (libewf) doğrulama.

Bütün testler **gerçek kod yollarını** çalıştırır; mock kullanılmaz.

---

## 2. Testleri Çalıştırma

```bash
# Bütün testler
cargo test --workspace

# Yalnızca çekirdek kütüphane
cargo test -p delilx-core

# E01 uyumluluk testleri (libewf gerekir)
cargo test -p delilx-core --test e01_compat

# Ayrıntılı çıktı
cargo test --workspace -- --nocapture
```

---

## 3. Birim Testleri

Her modül, kendi birim testlerini içerir:

| Modül | Test Edilen Davranış |
|---|---|
| `hash` | Bilinen vektörler (SHA-256, SHA-512), artımlı/tek geçiş tutarlılığı, doğrulama |
| `progress` | İptal belirteci, bayt/süre biçimlendirme, ilerleme oranı |
| `timeutil` | FILETIME dönüşümü, E01 tarih biçimi |
| `metadata` | Metadata okuma, envanter sayımı |
| `device` | Yazma koruması etiketleri, kaynak/hedef reddi |
| `source` | Dosya okuma, dosya sonu davranışı |
| `raw` | İmaj üretimi, iptal |
| `e01` | adler32, bölüm türü, segment yolu, sıkıştırma etiketleri |
| `fs` | NTFS/FAT imza tespiti, MBR ayrıştırma |
| `fs_fat` | Kısa ad (8.3) ve LFN çözümleme |
| `logical` | Kopyalama, uzantı filtresi, klasör yapısı koruma |
| `inspect` | RAW açma, dosya çıkarma |
| `verify` | Eşleşme ve bozulma tespiti |
| `report` | JSON/CSV/metin çıktısı, Türkçe içerik |
| `report_pdf` | Geçerli PDF üretimi, metin sarma |
| `acquisition` | Silinmiş veri uyarısı, ön kontroller |

**Toplam: 51 birim testi.**

---

## 4. Entegrasyon Testleri

`tests/integration.rs` dosyası uçtan uca akışları test eder:

| Test | Doğrulanan |
|---|---|
| `raw_imaj_uctan_uca_dogrulanir` | RAW imaj birebir kopya + hash doğrulama |
| `e01_imaj_icerik_aynen_geri_okunur` | E01 yaz→oku→karşılaştır |
| `e01_hash_degerleri_kaynakla_eslesir` | E01 MD5/SHA-1 = kaynak hash |
| `mantiksal_edinim_yapiyi_korur_ve_hash_uretir` | Klasör yapısı + hash |
| `hash_dogrulama_bozulmayi_yakalar` | Bozulma tespiti |
| `kaynak_hedef_ayni_cihaz_reddedilir` | Kaynak/hedef koruması |
| `korumasiz_kaynak_reddedilir` | Yazma koruması kontrolü |
| `yarim_kalan_islem_iptal_edilebilir` | İptal mekanizması |
| `imaj_inceleme_bolum_ve_dosya_listeler` | İnceleme akışı |
| `metadata_hash_ayri_degerlendirilir` | Hash/metadata ayrımı |
| `kullanilan_alan_silinen_veri_uyarisi_icerir` | Silinmiş veri uyarısı |

**Toplam: 11 entegrasyon testi.**

---

## 5. E01 Uyumluluk Testleri

`tests/e01_compat.rs` dosyası, üretilen E01 imajlarının **bağımsız doğrulama
araçlarıyla** (libewf) okunabildiğini denetler. Araçlar kurulu değilse testler
otomatik olarak atlanır.

| Test | Doğrulanan |
|---|---|
| `ewfinfo_delilx_imajini_encase6_olarak_tanir` | EnCase 6 biçimi, delil/vaka numaraları |
| `ewfverify_delilx_imajini_basarili_dogrular` | `ewfverify: SUCCESS` |
| `ewfverify_sikistirmasiz_imaji_dogrular` | Sıkıştırmasız mod |
| `ewfverify_cok_segmentli_imaji_dogrular` | Çok segmentli imaj |
| `ewfexport_ile_veri_birebir_cikarilir` | `ewfexport` ile veri çıkarma |
| `libewf_referans_imaji_okunabilir` | libewf imajını okuma (ters yön) |
| `delilx_ve_libewf_md5_uyusur` | MD5 eşleşmesi |

**Toplam: 7 E01 uyumluluk testi.**

### 5.1 Bağımsız Araç Kurulumu (Linux)

```bash
sudo apt-get install -y ewf-tools
```

Araçlar: `ewfinfo`, `ewfverify`, `ewfacquire`, `ewfexport`.

### 5.2 Bağımsız Doğrulama Örneği

```bash
# DelilX ile E01 üret
delilx e01 --kaynak kaynak.img --hedef cikti.E01 --delil-no DELIL-1

# Bağımsız araçla incele (EnCase 6 olarak tanınmalı)
ewfinfo cikti.E01

# Bağımsız araçla doğrula (SUCCESS döndürmeli)
ewfverify cikti.E01

# Bağımsız araçla veriyi çıkar ve karşılaştır
ewfexport -q -u -f raw -t cikarilan cikti.E01
```

---

## 6. Test Sonuçları

```
$ cargo test --workspace

test result: ok. 51 passed; 0 failed   (delilx-core birim testleri)
test result: ok. 11 passed; 0 failed   (entegrasyon testleri)
test result: ok.  7 passed; 0 failed   (E01 uyumluluk testleri)
```

**Toplam: 69 test, tamamı başarılı.**

---

## 7. Kalite Kontrolleri

Sürekli entegrasyon (GitHub Actions) şunları çalıştırır:

| Kontrol | Komut |
|---|---|
| Biçim denetimi | `cargo fmt --all -- --check` |
| Clippy (uyarılar hata) | `cargo clippy --workspace --all-targets -- -D warnings` |
| Testler | `cargo test --workspace` |
| Windows x64 derlemesi | `cargo build --release --target x86_64-pc-windows-gnu` |

Bütün testler başarılı olmadan yayın oluşturulmaz.

---

## 8. Test Verisi Üretimi

Testler yapay (deterministik veya PRNG ile üretilmiş) veri kullanır:

- **Deterministik veri:** Tekrarlanabilir sonuçlar için.
- **Yüksek entropili veri:** Sıkıştırılamayan blokları test etmek için
  (xorshift PRNG).
- **FAT imajları:** `mkfs.vfat` ve `mtools` ile üretilir.

### FAT Test İmajı Üretimi

```bash
dd if=/dev/zero of=fat.img bs=1M count=8
mkfs.vfat -F 32 -n TESTVOL fat.img
echo "içerik" > /tmp/dosya.txt
mcopy -i fat.img /tmp/dosya.txt ::DOSYA.TXT
```

---

## 9. Destrüktif Test Politikası

- Testler **yalnızca geçici dizinlerde** ve yapay verilerle çalışır.
- Gerçek USB belleklere veya SD kartlara test amacıyla bile yazma yapılmaz.
- Gerçek aygıt testleri yalnızca sanal disklerde yapılır.

---

## 10. Bilinen Sınırlamalar

Aşağıdaki özellikler bu sürümde sınırlıdır ve **tamamlanmış gibi
gösterilmemektedir**:

- **NTFS/exFAT dosya listeleme:** Bölüm ve dosya sistemi tespit edilir;
  ancak dizin ağacı çözümleme bu sürümde yalnızca FAT12/16/32 için
  uygulanmıştır. NTFS/exFAT için dosya listesi çıkarılamaz; bu durum
  kullanıcıya açıkça bildirilir.
- **Kullanılan alan edinimi:** Bölüm sınırlarına dayalı basit bir yaklaşım
  kullanır (bölümler arası boşluk atlanır). Dosya sistemi düzeyinde boş alan
  haritalama (bitmap) henüz uygulanmamıştır.
- **Cihaz algılama:** Yalnızca Windows üzerinde tam çalışır. Diğer
  platformlarda cihaz listesi boş döner (test ve geliştirme kolaylığı için).

Bu sınırlamalar [DEGISIKLIKLER.md](DEGISIKLIKLER.md) ve
[README.md](README.md) dosyalarında da belirtilmiştir.
