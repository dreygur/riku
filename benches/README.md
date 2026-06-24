# Riku Benchmarks

This directory contains benchmarking tools to measure Riku's performance and resource usage.

## Why Benchmark?

I believe in **transparency and verifiable data**. Instead of making unverified performance claims, we provide tools for you to:

1. Measure Riku's actual resource usage on your system
2. Validate that Riku meets your requirements
3. Share your findings with the community
4. Make informed decisions based on real data

## Acknowledgments

Riku is a Rust port of the excellent [Piku](https://github.com/piku/piku) micro-PaaS. Both projects share the same goal: making deployments simple and accessible for everyone. I encourage users to explore both projects and choose what works best for their needs.

## Quick Start

```bash
# Run the benchmark suite
cd benchmarks
chmod +x benchmark.sh
./benchmark.sh
```

### Binary Information
- Binary location
- Binary size
- Version

### Resource Usage
- Supervisor daemon memory
- Per-process memory
- CPU usage (when running)

### Performance
- CLI startup time
- Command execution time

### System Information
- OS version
- Architecture
- Available resources

## Contributing Benchmarks

I encourage users to share their benchmark results! Please submit:

1. **System specs** (CPU, RAM, OS, storage type)
2. **Workload description** (number of apps, traffic, etc.)
3. **Measured values** (memory, CPU, startup time)

Create a markdown file in `benches/results/` with your findings.

### Example Result Format

```markdown
# Benchmark Results - [Your Name/Company]

## Date
2026-02-16

## System Specifications
- **CPU**: Intel Xeon E5-2676 v3 @ 2.4GHz
- **RAM**: 2 GB
- **Storage**: SSD
- **OS**: Ubuntu 22.04 LTS

## Workload
- 3 small Node.js apps
- 2 Python Flask apps
- ~100 requests/minute total

## Results

### Binary Size
- Riku: 8.2 MB
- Piku (Python): ~25 MB (with dependencies)

### Memory Usage (Idle)
- Riku supervisor: 18 MB
- Piku processes: ~45 MB

### Startup Time
- Riku CLI: 12 ms
- Piku CLI: 450 ms

## Notes
[Your observations and findings]
```

## Disclaimer

Benchmark results vary based on:
- System hardware and configuration
- Number and type of deployed applications
- Traffic and workload patterns
- System load and other running processes

**Always run your own benchmarks** to validate performance for your specific use case.

## Tools Used

- `ps` - Process status
- `time` - Command execution timing
- `free` - Memory information
- `stat` - File size information
- `pgrep` - Process lookup

## Questions?

Open an issue if you have questions about benchmarking or want to suggest improvements to this benchmarking tools.
