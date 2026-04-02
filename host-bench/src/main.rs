use std::{
    env,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::Instant,
};

use self_recorder_packet::{DataBlockPacker, PushResult};

const DEFAULT_CSV_PATH: &str = "test-data/тест.csv";
const DEFAULT_ITERATIONS: usize = 20;
const PAGE_SIZE: usize = 4096;
const BASE_INTERVAL_MS: u32 = 20;
const INTERLEAVE_RATIO: [u32; 2] = [1, 1];
const FREF_HZ: f32 = 24_000_000.0;

#[derive(Debug, Clone)]
struct Config {
    csv_path: PathBuf,
    iterations: usize,
}

#[derive(Debug, Default)]
struct Stats {
    dataset_loops: usize,
    sample_count: usize,
    block_count: usize,
    input_bytes: usize,
    packed_bytes: usize,
}

fn main() -> Result<(), String> {
    let config = parse_args()?;
    let samples = read_samples(&config.csv_path)?;

    if samples.is_empty() {
        return Err(format!(
            "Файл '{}' не содержит валидных значений",
            config.csv_path.display()
        ));
    }

    let start = Instant::now();

    let total = run_benchmark(&samples, config.iterations)?;

    let elapsed = start.elapsed();

    let elapsed_sec = elapsed.as_secs_f64();
    let samples_per_sec = total.sample_count as f64 / elapsed_sec;
    let input_mib_per_sec = total.input_bytes as f64 / (1024.0 * 1024.0) / elapsed_sec;
    let packed_mib_per_sec = total.packed_bytes as f64 / (1024.0 * 1024.0) / elapsed_sec;
    let compression_ratio = if total.input_bytes == 0 {
        0.0
    } else {
        total.packed_bytes as f64 / total.input_bytes as f64
    };

    println!("Host benchmark completed:");
    println!("\tfile: {}", config.csv_path.display());
    println!("\tmin iterations: {}", config.iterations);
    println!("\tactual loops: {}", total.dataset_loops);
    println!("\tsamples: {}", total.sample_count);
    println!("\tpages(blocks): {}", total.block_count);
    println!("\tinput bytes: {}", total.input_bytes);
    println!("\tpacked bytes: {}", total.packed_bytes);
    println!("\tcompression ratio (packed/input): {:.4}", compression_ratio);
    println!("\telapsed: {:.3} s", elapsed_sec);
    println!("\tthroughput: {:.0} samples/s", samples_per_sec);
    println!("\tinput throughput: {:.3} MiB/s", input_mib_per_sec);
    println!("\tpacked throughput: {:.3} MiB/s", packed_mib_per_sec);

    Ok(())
}

fn run_benchmark(samples: &[u32], min_loops: usize) -> Result<Stats, String> {
    let mut stats = Stats {
        dataset_loops: 0,
        ..Stats::default()
    };

    let mut prev_value = 0_u32;
    let mut page_id = 0_u32;
    let mut packer = create_packer(page_id);

    const MAX_LOOPS_SAFETY: usize = 50_000;
    while stats.dataset_loops < min_loops || stats.block_count == 0 {
        for &sample in samples {
            stats.sample_count += 1;
            stats.input_bytes += std::mem::size_of::<u32>();

            let diff = sample as i32 - prev_value as i32;
            prev_value = sample;

            match packer.push_val(diff) {
                PushResult::Success => {}
                PushResult::Full => {
                    let packed = finalize_block(packer)?;
                    stats.block_count += 1;
                    stats.packed_bytes += packed.len();

                    page_id = page_id.saturating_add(1);
                    packer = create_packer(page_id);
                }
                PushResult::Overflow => {
                    return Err("DataBlockPacker overflow during benchmark".to_string());
                }
                PushResult::Finished => {
                    return Err("Unexpected finished state from DataBlockPacker".to_string());
                }
            }
        }

        stats.dataset_loops += 1;
        if stats.dataset_loops >= MAX_LOOPS_SAFETY {
            return Err("Не удалось получить ни одной полной страницы: достигнут safety-limit по количеству прогонов".to_string());
        }
    }

    Ok(stats)
}

fn create_packer(page_id: u32) -> DataBlockPacker {
    DataBlockPacker::builder()
        .set_ids(page_id.saturating_sub(1), page_id)
        .set_size(PAGE_SIZE)
        .set_timestamp((page_id as u64) * BASE_INTERVAL_MS as u64)
        .set_fref(FREF_HZ)
        .set_write_cfg(BASE_INTERVAL_MS, INTERLEAVE_RATIO)
        .build()
}

fn finalize_block(packer: DataBlockPacker) -> Result<Vec<u8>, String> {
    packer
        .to_result_full(|data| !crc32fast::hash(data))
        .ok_or_else(|| "Failed to finalize packed block".to_string())
}

fn read_samples(path: &Path) -> Result<Vec<u32>, String> {
    let file = File::open(path).map_err(|e| format!("Не удалось открыть '{}': {e}", path.display()))?;
    let reader = BufReader::new(file);

    let mut samples = Vec::new();

    for (line_idx, line_result) in reader.lines().enumerate() {
        let line = line_result.map_err(|e| {
            format!(
                "Ошибка чтения '{}' на строке {}: {e}",
                path.display(),
                line_idx + 1
            )
        })?;

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let value_hz: f32 = trimmed.parse().map_err(|e| {
            format!(
                "Некорректное число '{}' в '{}' на строке {}: {e}",
                trimmed,
                path.display(),
                line_idx + 1
            )
        })?;

        let scaled = (value_hz * 1000.0).round();
        if !(0.0..=(u32::MAX as f32)).contains(&scaled) {
            return Err(format!(
                "Значение вне диапазона u32 в '{}' на строке {}",
                path.display(),
                line_idx + 1
            ));
        }

        samples.push(scaled as u32);
    }

    Ok(samples)
}

fn parse_args() -> Result<Config, String> {
    let mut args = env::args().skip(1);

    let mut csv_path = PathBuf::from(DEFAULT_CSV_PATH);
    let mut iterations = DEFAULT_ITERATIONS;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-i" | "--iterations" => {
                let value = args.next().ok_or_else(|| {
                    "Ожидалось значение после --iterations/-i".to_string()
                })?;
                iterations = value.parse::<usize>().map_err(|e| {
                    format!("Некорректное значение iterations '{}': {e}", value)
                })?;
                if iterations == 0 {
                    return Err("iterations должно быть > 0".to_string());
                }
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            value if value.starts_with('-') => {
                return Err(format!("Неизвестный аргумент: {value}"));
            }
            path => {
                csv_path = PathBuf::from(path);
            }
        }
    }

    Ok(Config {
        csv_path,
        iterations,
    })
}

fn print_help() {
    println!("Host benchmark for STM32 self-writer packet pipeline");
    println!("\nUsage:");
    println!("  cargo run --release --target x86_64-pc-windows-msvc -- [CSV_PATH] [--iterations N]");
    println!("\nDefaults:");
    println!("  CSV_PATH     {}", DEFAULT_CSV_PATH);
    println!("  iterations   {}", DEFAULT_ITERATIONS);
}
