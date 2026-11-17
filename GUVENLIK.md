# DelilX Güvenlik Dokümantasyonu

Bu doküman DelilX'in güvenlik ilkelerini, kaynak bellek korumasını ve veri
bütünlüğü yaklaşımını açıklar.

> **Projenin temel ilkesi:** Kaynak delili koru, edinimi doğrula, bütün
> işlemleri kaydet ve kullanıcıya her şeyi Türkçe anlat.

---

## 1. Kaynak Bellek Koruma (En Önemli Gereksinim)

### 1.1 Kaynağa Yazma Yapılmaz

- Kaynak cihazlar **yalnızca okuma (`GENERIC_READ`)** erişimiyle açılır.
- Yazma erişimi (`GENERIC_WRITE`) hiçbir kod yolunda istenmez.
- Kaynak hiçbir koşulda otomatik biçimlendirilmez veya onarılmaz.
- Kaynak üzerinde dosya sistemi onarımı, sürücü mektubu değişikliği veya
  benzeri hiçbir işlem yapılmaz.

### 1.2 Cihaz Algılama ve Ayrım

- USB bellekler ve SD kartlar otomatik algılanır.
- Fiziksel disk bilgileri gösterilir: kapasite, seri numarası, dosya sistemi,
  fiziksel sürücü numarası.
- Kaynak ve hedef diskler birbirinden ayrılır.

### 1.3 Kaynak/Hedef Doğrulaması

Edinim başlatılmadan önce şu kontroller yapılır:

| Kontrol | Sonuç |
|---|---|
| Kaynak ve hedef aynı fiziksel sürücüde mi? | Reddedilir |
| Hedef, kaynağın birim harfi altında mı? | Reddedilir |
| Kaynakta doğrulanmış yazma koruması var mı? | Yoksa reddedilir |

Bu kontrollerden biri başarısız olursa edinim başlatılmaz ve kullanıcı Türkçe
olarak uyarılır.

### 1.4 Yazma Koruması Ayrımı

DelilX, yazma koruması türlerini **birbirinden ayrı** değerlendirir:

| Tür | Açıklama |
|---|---|
| **Donanımsal yazma engelleyici** | Fiziksel write blocker veya salt okunur anahtar |
| **Yazılımsal salt okunur** | Yalnızca kilit ile açılmış; donanımsal değil |
| **Doğrulanamadı** | Sorgu başarısız |
| **Yok** | Aygıt yazılabilir |

> **Önemli:** Yazılımsal salt okunur koruma, donanımsal yazma engelleyiciyle
> **eşdeğer gösterilmez.** Adli edinim için donanımsal yazma engelleyici
> şiddetle önerilir.

Yazma koruması, `IOCTL_DISK_IS_WRITABLE` ile denetlenir. Sorgu
`ERROR_WRITE_PROTECT` (19) ile başarısız olursa donanımsal koruma olarak
değerlendirilir.

---

## 2. Veri Bütünlüğü

### 2.1 Hash Doğrulama

- **SHA-256** ve **SHA-512** her edinimde hesaplanır.
- E01 imajlarında ayrıca **MD5** ve **SHA-1** hesaplanır ve imajın içine yazılır.
- Edinim sonrası hedef imaj yeniden okunarak hash değerleri karşılaştırılır.

### 2.2 Hash ve Metadata Ayrımı

Hash değerleri ile metadata bütünlüğü **ayrı değerlendirilir**:

- **Hash bütünlüğü:** Verinin bit düzeyinde bozulmadığını gösterir.
- **Metadata bütünlüğü:** Zaman damgaları, öznitelikler ve dosya sistemi
  üst verisinin korunmasını gösterir.

Hash eşleşmesi metadata bütünlüğünü garanti etmez. Korunamayan metadata
alanları raporda **açıkça belirtilir**.

### 2.3 Bütünlük Denetimi

- Dosya boyutu kontrolü (kaynak ve hedef).
- E01 blok adler32 sağlamaları doğrulanır.
- Okuma hatası kaydı: hatalı sektörlerin numarası ve açıklaması kaydedilir.

---

## 3. İşlem Kayıtları

Bütün işlemler zaman damgalı olarak kaydedilir:

- Edinim başlangıç ve bitiş zamanları.
- Okuma hataları ve yeniden denemeler.
- Doğrulama sonuçları.
- Uyarılar ve hatalar.

İşlem kayıtları hem arayüzde gösterilir hem de rapora dahil edilir.

---

## 4. Hata Yönetimi

### 4.1 Okuma Hataları

Kullanıcı politikayı seçer:

- **Sıfırla ve devam et (ZeroFill):** Hatalı sektörler sıfırlarla doldurulur;
  işlem sürer. Hatalı sektör numaraları kaydedilir.
- **Durdur (Abort):** İlk okuma hatasında işlem durur.
- **En fazla N hata:** Belirtilen sayıda hatadan sonra durur.

### 4.2 Yazma Koruması Doğrulanamazsa

İşlem başlatılmaz; kullanıcı Türkçe olarak uyarılır.

### 4.3 Kaynak/Hedef Karışıklığı

Aynı aygıt seçilirse işlem reddedilir ve kullanıcı bilgilendirilir.

---

## 5. Destrüktif İşlemler

DelilX **hiçbir destrüktif işlem yapmaz**:

- Kaynağa yazmaz.
- Biçimlendirmez.
- Onarmaz.
- Silmez.

Testler yalnızca sanal disklerde ve geçici test verilerinde çalıştırılır.
Gerçek aygıtlara test amacıyla bile yazma yapılmaz.

---

## 6. Gizlilik ve Ağ

- Uygulama **internet bağlantısı olmadan** çalışır.
- Hiçbir veri ağ üzerinden gönderilmez.
- Hiçbir telemetri veya uzak günlükleme yoktur.
- Bütün veriler yerel diske yazılır.

---

## 7. Silinmiş Veri Uyarısı

| Edinim Yöntemi | Silinmiş Veri |
|---|---|
| RAW / DD | **Korunur** (tam sektör kopyası) |
| E01 | **Korunur** (tam sektör kopyası) |
| Mantıksal | Kaybolur (yalnızca mevcut dosyalar) |
| Seçmeli | Kaybolur (yalnızca seçilen dosyalar) |
| Kullanılan Alan | **Kaybolur** (boş alan atlanır) |

Kullanılan alan ediniminde kullanıcı, silinmiş verilerin kaybolacağı konusunda
**açıkça uyarılır**; bu uyarı raporda da yer alır.

---

## 8. E01 Biçim Güvenliği

- E01 imajları, içerdikleri verinin MD5 ve SHA-1 değerlerini taşır.
- Blok düzeyinde adler32 sağlamaları ile bozulma tespit edilir.
- İmaj bütünlüğü bağımsız araçlarla (`ewfverify`) doğrulanabilir.

---

## 9. Girdi Doğrulama

- Kaynak ve hedef yolları doğrulanır.
- Sektör boyutu (512/4096) ve blok boyutu sınırlandırılır.
- Dosya boyutu ve ofset taşmalarına karşı kontroller yapılır.
- E01 bölüm boyutları ve sağlamaları doğrulanır; bozuk dosyalar reddedilir.

---

## 10. Raporlama ve İzlenebilirlik

Her rapor şunları içerir:

- İşlem numarası ve delil numarası.
- İncelemeyi yapan kişi.
- Kaynak cihaz bilgileri (seri numarası, kapasite, dosya sistemi).
- Edinim yöntemi.
- Başlangıç ve bitiş zamanı.
- Hash değerleri ve doğrulama sonucu.
- Hata ve uyarılar.

Bu, adli bilişimde **zincir-i delil (chain of custody)** gereksinimlerine
uygun izlenebilirlik sağlar.

---

## 11. Sorumlu Kullanım

DelilX bir adli bilişim aracıdır. Kullanıcı:

- Yalnızca yetkili olduğu cihazlarda edinim yapmalıdır.
- Adli edinim için donanımsal yazma engelleyici kullanmalıdır.
- Edinim sonrası hash değerlerini bağımsız araçlarla doğrulamalıdır.
- İşlem kayıtlarını ve raporları saklamalıdır.

---

## 12. Güvenlik Kontrol Listesi

Edinim öncesi:

- [ ] Donanımsal yazma engelleyici bağlı.
- [ ] Kaynak ve hedef farklı fiziksel aygıtlarda.
- [ ] Hedef diskte yeterli boş alan var.
- [ ] Delil ve vaka numaraları girildi.

Edinim sonrası:

- [ ] Doğrulama **BAŞARILI** döndü.
- [ ] Hash değerleri kaydedildi.
- [ ] Rapor (PDF/JSON/CSV) oluşturuldu.
- [ ] E01 imajları bağımsız araçla doğrulandı.
