# DelilX Kullanım Kılavuzu

Bu kılavuz DelilX'in grafik arayüzünü ve komut satırı aracını kullanmayı
adım adım açıklar.

---

## İçindekiler

1. [Genel Bakış](#1-genel-bakış)
2. [Cihazları Algıla](#2-cihazları-algıla)
3. [RAW İmaj Edinimi](#3-raw-imaj-edinimi)
4. [E01 İmaj Edinimi](#4-e01-imaj-edinimi)
5. [Mantıksal Edinim](#5-mantıksal-edinim)
6. [Seçmeli Edinim](#6-seçmeli-edinim)
7. [Kullanılan Alan Edinimi](#7-kullanılan-alan-edinimi)
8. [İmaj İnceleme](#8-imaj-inceleme)
9. [Hash Doğrulama](#9-hash-doğrulama)
10. [Delil Raporları](#10-delil-raporları)
11. [Ayarlar](#11-ayarlar)
12. [Komut Satırı Aracı](#12-komut-satırı-aracı)

---

## 1. Genel Bakış

DelilX'in temel ilkesi şudur:

> **Kaynak delili koru, edinimi doğrula, bütün işlemleri kaydet ve kullanıcıya
> her şeyi Türkçe anlat.**

Sol menüden ekranlar arasında geçiş yapabilirsiniz. Alt durum çubuğu işlemin
durumunu ve ilerlemesini gösterir. Uzun süren işlemler arka planda çalışır;
arayüz donmaz ve işlemi dilediğiniz zaman iptal edebilirsiniz.

---

## 2. Cihazları Algıla

USB bellekler ve SD kartlar otomatik algılanır.

1. Sol menüden **Cihazları Algıla** seçin.
2. **Cihazları Yeniden Tara** düğmesine basın.
3. Listede şunları görürsünüz:
   - Aygıt adı
   - Tür (Sabit Disk, Çıkarılabilir Aygıt, vb.)
   - Kapasite
   - Dosya sistemi (NTFS, FAT32, exFAT)
   - **Yazma Koruması** durumu
4. Bir aygıtı işaretleyin. Ayrıntılar alt bölümde görünür.
5. **Bu Aygıtı Kaynak Olarak Seç** düğmesine basın.

> **Güvenlik:** Yazma koruması "Doğrulanamadı" veya "Yok" ise DelilX edinimi
> başlatmaz. Donanımsal yazma engelleyici bağlayın.

---

## 3. RAW İmaj Edinimi

RAW/DD imaj, kaynağın tam sektör kopyasıdır. **Silinmiş veriler ve boş alan
korunur.**

1. **RAW İmaj** ekranını açın.
2. **Kaynak** alanına cihaz yolunu girin (`\\.\PhysicalDrive2`) veya **Gözat**
   ile seçin.
3. **Hedef** alanına imaj dosyasını girin (`D:\delil\usb.dd`).
4. Sektör boyutunu (512 veya 4096) seçin.
5. Okuma hatası politikasını seçin:
   - **Sıfırla ve devam et:** Hatalı sektörler sıfırlarla doldurulur, işlem sürer.
   - **Durdur:** İlk okuma hatasında işlem durur.
6. **Edinim sonrası doğrula** seçeneğini işaretli bırakın (önerilir).
7. **RAW İmajı Oluştur** düğmesine basın.

İşlem tamamlandığında okunan bayt, sektör sayısı, okuma hataları ve hash
değerleri gösterilir. Doğrulama sonucu **BAŞARILI** veya **BAŞARISIZ** olarak
belirtilir.

---

## 4. E01 İmaj Edinimi

E01 imaj, EnCase uyumlu sıkıştırılmış imaj biçimidir.

1. **E01 İmaj** ekranını açın.
2. Kaynak ve hedef yollarını girin (hedef: `D:\delil\usb.E01`).
3. Delil ve uzman bilgilerini doldurun:
   - Delil numarası
   - Vaka numarası
   - İncelemeyi yapan
   - Açıklama, notlar
4. Sıkıştırmayı seçin (**Yok**, **Hızlı**, **En iyi**).
5. Segment boyutunu (MiB) belirleyin. Büyük imajlar otomatik olarak parçalara
   ayrılır (`.E01`, `.E02`, ...).
6. **E01 İmajı Oluştur** düğmesine basın.

Sonuçta segment dosyaları, toplam boyut, blok sayısı, **MD5** ve **SHA-1**
değerleri gösterilir. Bu değerler imajın içine yazılır ve bağımsız araçlarla
(`ewfverify`) doğrulanabilir.

---

## 5. Mantıksal Edinim

Yalnızca mevcut dosyaları alır; klasör yapısını korur.

1. **Mantıksal Edinim** ekranını açın.
2. **Kaynak dizin** ve **hedef dizin** seçin.
3. İsteğe bağlı olarak uzantı filtresi girin (ör. `txt,pdf,jpg`).
4. **Yalnızca envanter** seçeneğini işaretlerseniz dosyalar kopyalanmaz,
   yalnızca envanter ve hash listesi üretilir.
5. **Mantıksal Edinimi Başlat** düğmesine basın.

Hedef dizinde `envanter.csv` ve `hash_listesi.csv` dosyaları oluşturulur.

> **Uyarı:** Mantıksal edinim yalnızca mevcut dosyaları alır. Silinmiş
> dosyalar, boş alan kalıntıları ve dosya sistemi üst verisi edinilmez.

---

## 6. Seçmeli Edinim

Kullanıcının seçtiği dosya ve klasörleri alır.

1. **Seçmeli Edinim** ekranını açın.
2. **Kaynak dizin** ve **hedef dizin** seçin.
3. **Dosya Ekle** ile belirli dosyaları listeye ekleyin.
4. **Seçmeli Edinimi Başlat** düğmesine basın.

Seçilen dosyalar kopyalanır, dosya envanteri ve hash listesi oluşturulur.

---

## 7. Kullanılan Alan Edinimi

Yalnızca dosya sisteminin kullandığı sektörleri alır.

1. **Kullanılan Alan** ekranını açın.
2. Kaynak ve hedef yollarını girin.
3. **Kullanılan Alanı Edin** düğmesine basın.

Sonuçta tespit edilen bölümler, dosya sistemi, edinilen bayt ve atlanan boş
alan gösterilir.

> **Kritik Uyarı:** Boş alan dışarıda bırakıldığı için **silinmiş dosyalar ve
> boş alandaki kalıntı veriler imaja dahil edilmez ve kurtarılamaz.** Adli
> incelemede silinmiş veri önemliyse tam disk imajı (RAW veya E01) tercih
> edilmelidir.

---

## 8. İmaj İnceleme

Alınan RAW ve E01 imajları açar.

1. **İmaj İnceleme** ekranını açın.
2. İmaj dosyasını seçin (`.dd`, `.img`, `.E01`).
3. **İmajı İncele** düğmesine basın.

Sonuçta şunlar gösterilir:
- İmaj türü ve boyutu
- Bölüm tablosu türü (MBR/GPT) ve bölümler
- Her bölümün dosya sistemi
- Dosya listesi (FAT12/16/32 için)

> **Not:** Orijinal imaj dosyası hiçbir koşulda değiştirilmez.

---

## 9. Hash Doğrulama

Bir imaj veya dosyanın hash değerlerini yeniden hesaplayarak karşılaştırır.

1. **Hash Doğrulama** ekranını açın.
2. Dosyayı seçin.
3. Beklenen **SHA-256** ve/veya **SHA-512** değerlerini girin.
4. **Doğrula** düğmesine basın.

Sonuç **BAŞARILI** veya **BAŞARISIZ** olarak gösterilir.

> **Not:** Hash değerleri ile metadata bütünlüğü ayrı değerlendirilir. Hash
> eşleşmesi veri bütünlüğünü gösterir; metadata bütünlüğü ayrıca incelenmelidir.

---

## 10. Delil Raporları

Edinim tamamlandıktan sonra Türkçe rapor oluşturur.

1. **Delil Raporları** ekranını açın.
2. İşlem numarası, delil numarası, vaka numarası, incelemeyi yapan ve açıklama
   alanlarını doldurun.
3. Çıktı dizinini seçin.
4. **Rapor Oluştur (PDF, JSON, CSV)** düğmesine basın.

Rapor şu bilgileri içerir:
- İşlem numarası, delil numarası, incelemeyi yapan
- Kaynak cihaz bilgileri
- Edinim yöntemi, başlangıç/bitiş zamanı, süre
- Dosya sayısı, veri miktarı
- Hash değerleri ve doğrulama sonucu
- Hata ve uyarılar

Üç dosya oluşturulur: `delil_raporu_<işlem>.pdf`, `.json`, `.csv`.

---

## 11. Ayarlar

- **Varsayılan delil bilgileri:** Delil numarası, vaka numarası, incelemeyi yapan.
- **Varsayılan sektör boyutu:** 512 veya 4096 bayt.
- **Hash algoritmaları:** SHA-256, SHA-512.
- **Doğrulama:** Edinim sonrası otomatik doğrulama.

> E01 imajları biçim gereği her zaman MD5 ve SHA-1 değerlerini de içerir.

---

## 12. Komut Satırı Aracı

`delilx-cli.exe` komut satırı aracı, grafik arayüzün yaptığı işlemleri komut
satırından yapmaya olanak tanır.

### Komutlar

```text
delilx-cli hakkinda                              # Sürüm ve slogan
delilx-cli cihazlar                              # Depolama aygıtlarını listeler
delilx-cli raw     --kaynak <YOL> --hedef <YOL>  # RAW imaj oluşturur
delilx-cli e01     --kaynak <YOL> --hedef <YOL>  # E01 imaj oluşturur
delilx-cli mantiksal --kaynak <DİZİN> --hedef <DİZİN>  # Mantıksal edinim
delilx-cli incele  --imaj <YOL>                  # İmajı inceler
delilx-cli dogrula --imaj <YOL> [--sha256 <DEĞER>] [--sha512 <DEĞER>]
```

### Örnekler

```powershell
# RAW imaj al
delilx-cli raw --kaynak "\\.\PhysicalDrive2" --hedef "D:\delil\usb.dd"

# E01 imaj al (delil bilgileri ile)
delilx-cli e01 --kaynak "\\.\PhysicalDrive2" --hedef "D:\delil\usb.E01" `
           --delil-no "DELIL-2026-001" --vaka-no "VAKA-2026-042" `
           --uzman "Ad Soyad" --sikistirma eniyi

# Mantıksal edinim
delilx-cli mantiksal --kaynak "C:\Users\kullanici\Belgeler" --hedef "D:\delil\belgeler"

# İmajı incele
delilx-cli incele --imaj "D:\delil\usb.E01"

# Hash doğrula
delilx-cli dogrula --imaj "D:\delil\usb.dd" --sha256 "e3b0c44298fc1c14..."
```

Bütün çıktılar Türkçedir.

---

## Sık Sorulan Sorular

**S: Kaynak cihaza yazma yapılır mı?**
C: Hayır. DelilX kaynağa hiçbir koşulda yazmaz; yalnızca okuma erişimi kullanır.

**S: Yazılımsal salt okunur yeterli mi?**
C: Hayır. Yazılımsal koruma, donanımsal yazma engelleyiciyle eşdeğer değildir.
Adli edinim için donanımsal yazma engelleyici kullanın.

**S: E01 imajım gerçekten EnCase uyumlu mu?**
C: Evet. DelilX'in ürettiği E01 imajları bağımsız libewf araçlarıyla
(`ewfinfo`, `ewfverify`) doğrulanır.

**S: Kullanılan alan ediniminde silinmiş veriler alınır mı?**
C: Hayır. Boş alan dışarıda bırakıldığı için silinmiş veriler kaybolur. Tam
disk imajı kullanın.
