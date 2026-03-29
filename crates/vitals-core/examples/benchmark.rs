use std::sync::Arc;
use std::time::{Duration, Instant};

fn main() {
    let config = vitals_core::config::AppConfig::default();
    let config = Arc::new(config);
    let mut manager = vitals_core::sensors::SensorManager::new(&config);

    // Warmup - first poll (includes hardware discovery)
    let start = Instant::now();
    let readings = manager.query_all(0.0);
    let discovery_time = start.elapsed();
    println!(
        "Discovery + first poll: {:.3}ms ({} readings)",
        discovery_time.as_secs_f64() * 1000.0,
        readings.len()
    );

    // Benchmark 100 sensor polls
    let mut times = Vec::new();
    let mut total_readings = 0;
    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(10));
        let start = Instant::now();
        let readings = manager.query_all(0.01);
        let elapsed = start.elapsed();
        times.push(elapsed);
        total_readings += readings.len();
    }

    let total: Duration = times.iter().sum();
    let avg = total / times.len() as u32;
    let min = *times.iter().min().unwrap();
    let max = *times.iter().max().unwrap();
    times.sort();
    let p50 = times[49];
    let p99 = times[98];

    println!("\n=== Sensor Poll Latency (100 iterations) ===");
    println!("  Average:  {:.3}ms", avg.as_secs_f64() * 1000.0);
    println!("  Median:   {:.3}ms", p50.as_secs_f64() * 1000.0);
    println!("  Min:      {:.3}ms", min.as_secs_f64() * 1000.0);
    println!("  Max:      {:.3}ms", max.as_secs_f64() * 1000.0);
    println!("  P99:      {:.3}ms", p99.as_secs_f64() * 1000.0);
    println!("  Readings/poll: {}", total_readings / 100);

    // Value formatting benchmark
    let formatter = vitals_core::format::ValueFormatter::new(&config);
    let start = Instant::now();
    for _ in 0..10000 {
        let _ = formatter.format(45000.0, vitals_core::sensors::SensorFormat::Temp);
        let _ = formatter.format(0.45, vitals_core::sensors::SensorFormat::Percent);
        let _ = formatter.format(3_500_000_000.0, vitals_core::sensors::SensorFormat::Hertz);
        let _ = formatter.format(1_048_576.0, vitals_core::sensors::SensorFormat::Memory);
        let _ = formatter.format(15_000_000.0, vitals_core::sensors::SensorFormat::Watt);
    }
    let format_time = start.elapsed();
    println!("\n=== Value Formatting (50,000 calls) ===");
    println!(
        "  Total:    {:.3}ms",
        format_time.as_secs_f64() * 1000.0
    );
    println!(
        "  Per call: {:.3}us",
        format_time.as_secs_f64() * 1_000_000.0 / 50000.0
    );
}
