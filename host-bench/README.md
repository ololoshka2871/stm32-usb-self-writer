# Host benchmark (без ARM)

Бенчмарк запускается как отдельный хостовый crate и использует файл `test-data/тест.csv` по умолчанию.

```bash
cargo run --release --target x86_64-pc-windows-msvc -- path/to/data.csv
```
