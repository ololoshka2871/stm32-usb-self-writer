use heatshrink_rust::CompressedData;
use heatshrink_rust_macro::{packed_file, packed_string};

//-------------------------------------------------------------------------------------------------

pub(crate) static README_COMPRESSED: CompressedData = packed_string!(
    r#"# СКТБ "ЭЛПА": Автономный регистратор давления

Этот виртуальный диск предоставляет доступ к содержимому внутреннего накопителя устройства.

- Образ памяти устройсва отображается в файл data_raw.hs.
- Локальная часть памяти, занята данными отображается в файл файл data_use.hs.
- Для расшифровки содержимого используйте программу pt-self-writer-decoder.
- Драйвер для виртуального последовательного порта: driver.inf (Windows 7).
- Коэффициенты полиномов для рассчета находятся в файле config.var (формат json)
- Информация о занятой памяти в файле storage.var (формат json)
- Для управление функционалом устройства используйте программу KalibratorGUI
"#
);

//-------------------------------------------------------------------------------------------------
// пусть до файлов относительно каталога, содержащего Cargo.toml

pub(crate) static DRIVER_INF_COMPRESSED: CompressedData = packed_file!("stm32-USB-Self-writer.inf");

pub(crate) static PROTO_COMPRESSED: CompressedData =
    packed_file!("src/protobuf/ru.sktbelpa.protobufobjects/ProtobufDevice_0000E006.proto");
