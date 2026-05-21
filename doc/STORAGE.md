# STORAGE Plan

## Цели

1. Реализовать хранилище с записью блоками фиксированного размера (по умолчанию `4096` байт, кратно `FlashConfig.write_max_bytes`).
2. Поддержать 1 или 2 QSPI-flash, при 2 flash — «шахматное» распределение блоков.
3. Сохранить для клиента линейную модель блоков `0,1,2...` независимо от числа flash.
4. В режиме самописца: однократный стартовый scan, затем только запись; между операциями flash в sleep.
5. В режиме USB: чтение через VFS + memory-mapped оптимизация; разрешить только команду полного стирания.
6. Дать `Copy + Send` meta-handle для `/storage.var` и `protobuf_server()`.
7. Сделать storage API независимым от типа носителя: текущий backend QSPI и перспективный backend eMMC должны подключаться без изменения верхнего слоя.

---

## Зафиксированные правила

- **Block size**: `4096` байт (конфигурируемо, но обязательно кратно `write_max_bytes`).
- **Recorder mode**:
  - накопление данных на 1 блок в RAM;
  - запись готового блока;
  - flash спит в простое;
  - если 2 flash — запись по очереди между банками.
- **USB mode**:
  - при старте выполняется scan для определения `used_blocks`;
  - чтение только read-only;
  - доступна команда erase всего массива;
  - во время erase чтение `/storage.var`, `/data_raw.hs`, `/data_use.hs` возвращает `BlockDeviceError::NotReady`.
- **VFS files**:
  - `/data_raw.hs` = вся доступная память (`total_blocks * block_size`)
  - `/data_use.hs` = только занятые блоки (`used_blocks * block_size`)
  - различие только в параметрах `EntryBuilder`.
- **Расширяемость backend**:
  - слой `StorageCore` и meta/VFS API не зависит от транспорта/носителя;
  - backend реализует единый контракт (`scan/read/write/erase/sleep/mmap` capabilities);
  - миграция QSPI -> eMMC не требует изменений в `protobuf_server`, `EMfatStorage` и бизнес-логике режимов.

---

## Логический маппинг блоков

### 1 flash

- `bank = 0`
- `local_block = global_block`

### 2 flash (шахматка)

- `bank = global_block % 2`
- `local_block = global_block / 2`

### Маппинг смещения файла

- `global_block = offset / block_size`
- `in_block_offset = offset % block_size`
- `local_addr = local_block * block_size + in_block_offset`

---

## Архитектура

```mermaid
flowchart LR
    A[Recorder/USB logic] --> B[StorageCore]
    B --> C[BlockMapper]
  B --> D[StorageBackend trait]
  D --> E[QSPI backend]
  D --> I[eMMC backend]
    B --> F[StorageMetaHandle Copy + Send]

    G[VFS EMfatStorage] --> F
    H[protobuf_server] --> F

    G -->|read data_raw/use| B
    A -->|write blocks| B
```

```mermaid
classDiagram
    class StorageBackend {
      <<trait>>
      +geometry()
      +scan_used_blocks()
      +read_range(offset,len)
      +write_block(global_block,data)
      +erase_all()
      +set_sleep(enabled)
    }

    class StorageCore {
      +init(mode)
      +scan_used_blocks()
      +write_block(data)
      +read_range(offset,len)
      +erase_all()
      +set_mode(mode)
    }

    class BlockMapper {
      +map_block(global_id) bank,local_id
      +map_offset(offset) bank,local_addr
    }

    class StorageMetaHandle {
      <<Copy, Send>>
      +block_size()
      +total_blocks()
      +used_blocks()
      +is_busy()
      +erase_in_progress()
      +request_erase()
    }

    class VfsAdapter {
      +build_entries()
      +read_block(lba)
      +is_ready()
    }

    StorageCore --> StorageBackend
    StorageCore --> BlockMapper
    StorageCore --> StorageMetaHandle
    VfsAdapter --> StorageMetaHandle
```

---

## Состояния и переходы

```mermaid
stateDiagram-v2
    [*] --> Init
    Init --> ScanStartup
    ScanStartup --> RecorderReady: mode=Recorder
    ScanStartup --> UsbReady: mode=USB

    RecorderReady --> Buffering
    Buffering --> WritingBlock: block full
    WritingBlock --> SleepIdle
    SleepIdle --> Buffering

    UsbReady --> Reading
    Reading --> UsbReady

    UsbReady --> Erasing: erase command
    Erasing --> UsbReady: done + used_blocks=0

    Erasing --> ReadBlocked
    ReadBlocked --> UsbReady: erase done
```

---

## Последовательность операций

### Recorder mode

```mermaid
sequenceDiagram
    participant APP as Recorder Task
    participant CORE as StorageCore
    participant MAP as BlockMapper
    participant B1 as Flash#1
    participant B2 as Flash#2

    APP->>CORE: push measurements
    CORE->>CORE: accumulate to 1 block
    APP->>CORE: flush block
    CORE->>MAP: map(global_block)
    alt bank=0
      CORE->>B1: wake + write block
      CORE->>B1: sleep
    else bank=1
      CORE->>B2: wake + write block
      CORE->>B2: sleep
    end
    CORE->>CORE: used_blocks += 1
```

### USB mode read

```mermaid
sequenceDiagram
    participant HOST as USB Host
    participant VFS as EMfatStorage
    participant CORE as StorageCore
    participant MAP as BlockMapper

    HOST->>VFS: read /data_raw.hs (lba)
    VFS->>CORE: read_range(offset,len)
    CORE->>MAP: map_offset(offset)
    CORE-->>VFS: bytes
    VFS-->>HOST: bytes
```

### USB mode erase

```mermaid
sequenceDiagram
    participant HOST as USB Host/Command
    participant META as StorageMetaHandle
    participant CORE as StorageCore
    participant VFS as EMfatStorage

    HOST->>META: request_erase()
    META->>CORE: start erase job
    CORE->>CORE: erase_in_progress=true
    VFS-->>HOST: NotReady for storage files
    CORE->>CORE: erase banks
    CORE->>CORE: used_blocks=0
    CORE->>CORE: erase_in_progress=false
```

---

## План реализации (по шагам)

1. **Domain types и контракты**
  - Ввести `StorageMode`, `StorageError`, `StorageGeometry`, `StorageStats`.
   - Ввести `StorageMetaHandle` (`Copy + Send`) на атомиках.
  - Ввести `StorageBackend` trait как стабильный контракт для QSPI/eMMC.

2. **Mapper и геометрия**
   - Реализовать `BlockMapper` для 1/2 flash.
   - Вынести функции `map_block`, `map_offset`, `raw_size_bytes`, `used_size_bytes`.

3. **StorageCore**
  - Поднять core-объект с generic backend (`StorageBackend`).
   - Реализовать startup scan (`scan_used_blocks`) с поиском первого пустого блока.
   - Реализовать запись блока в recorder policy (wake/write/sleep).

4. **Erase pipeline (USB only)**
   - Реализовать `request_erase()` через meta-handle.
   - Во время erase поднимать `erase_in_progress`, по завершению `used_blocks=0`.

5. **VFS integration**
   - Вернуть `/storage.var`, `/data_raw.hs`, `/data_use.hs`.
   - Размеры файлов брать из meta-handle.
   - В `read_block` возвращать `BlockDeviceError::NotReady` для storage-файлов во время erase.

6. **Init integration**
   - Создавать `StorageCore` и `StorageMetaHandle` в `self_writer::app::init()`.
   - Передавать meta-handle в VFS и `protobuf_server`.

7. **Проверка и наблюдаемость**
   - Логи по mode/startup scan/map/erase transitions.
   - Sanity checks: границы адресов, выравнивание размеров, consistency `used <= total`.

---

## Критерии готовности (DoD)

- В recorder mode блоки пишутся поочередно по банкам (если 2 flash) и остаются линейными для клиента.
- В USB mode `data_raw.hs` и `data_use.hs` имеют корректные размеры.
- При erase чтение storage-файлов даёт `NotReady`.
- После erase `used_blocks == 0`.
- Meta-handle доступен из VFS и `protobuf_server`, не блокирует чтение.
- `cargo check --release --features maket,xtal-24mhz` проходит.
