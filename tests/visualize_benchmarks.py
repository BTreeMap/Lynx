#!/usr/bin/env python3
"""
Benchmark Results Visualizer

This script generates visual graphs from benchmark JSON results.
Requires: matplotlib, numpy

Usage:
    python3 visualize_benchmarks.py benchmark-results.json
"""

import json
import sys
import argparse
from pathlib import Path

try:
    import matplotlib.pyplot as plt
    import numpy as np
except ImportError:
    print("Error: Required packages not installed")
    print("Install with: pip3 install matplotlib numpy")
    sys.exit(1)


def load_results(json_file):
    """Load benchmark results from JSON file."""
    with open(json_file, 'r') as f:
        return json.load(f)


def plot_rps_comparison(data, output_dir):
    """Create bar chart comparing RPS across tests."""
    tests = [t['name'] for t in data['tests']]
    rps = [float(t['requests_per_second']) for t in data['tests']]
    
    plt.figure(figsize=(14, 8))
    bars = plt.bar(range(len(tests)), rps, color='steelblue', alpha=0.8)
    
    # Add value labels on bars
    for i, bar in enumerate(bars):
        height = bar.get_height()
        plt.text(bar.get_x() + bar.get_width()/2., height,
                f'{int(height):,}',
                ha='center', va='bottom', fontsize=9)
    
    plt.xlabel('Test Scenario', fontsize=12, fontweight='bold')
    plt.ylabel('Requests per Second', fontsize=12, fontweight='bold')
    plt.title('Lynx Performance Benchmarks - Throughput Comparison', 
              fontsize=14, fontweight='bold')
    plt.xticks(range(len(tests)), tests, rotation=45, ha='right')
    plt.tight_layout()
    plt.grid(axis='y', alpha=0.3)
    
    output_path = output_dir / 'rps_comparison.png'
    plt.savefig(output_path, dpi=150, bbox_inches='tight')
    print(f"✓ Saved: {output_path}")
    plt.close()


def plot_latency_percentiles(data, output_dir):
    """Create grouped bar chart for latency percentiles."""
    tests = [t['name'] for t in data['tests']]
    p50 = [float(t.get('p50_latency_ms', 0)) for t in data['tests']]
    p90 = [float(t.get('p90_latency_ms', 0)) for t in data['tests']]
    p99 = [float(t.get('p99_latency_ms', 0)) for t in data['tests']]
    
    x = np.arange(len(tests))
    width = 0.25
    
    plt.figure(figsize=(14, 8))
    plt.bar(x - width, p50, width, label='p50', color='lightgreen', alpha=0.8)
    plt.bar(x, p90, width, label='p90', color='orange', alpha=0.8)
    plt.bar(x + width, p99, width, label='p99', color='red', alpha=0.8)
    
    plt.xlabel('Test Scenario', fontsize=12, fontweight='bold')
    plt.ylabel('Latency (ms)', fontsize=12, fontweight='bold')
    plt.title('Lynx Performance Benchmarks - Latency Percentiles', 
              fontsize=14, fontweight='bold')
    plt.xticks(x, tests, rotation=45, ha='right')
    plt.legend(fontsize=10)
    plt.tight_layout()
    plt.grid(axis='y', alpha=0.3)
    
    output_path = output_dir / 'latency_percentiles.png'
    plt.savefig(output_path, dpi=150, bbox_inches='tight')
    print(f"✓ Saved: {output_path}")
    plt.close()


def plot_performance_heatmap(data, output_dir):
    """Create heatmap showing performance metrics."""
    tests = [t['name'][:40] for t in data['tests']]  # Truncate long names
    
    metrics = ['RPS', 'Avg Latency', 'p50', 'p90', 'p99']
    values = []
    
    for test in data['tests']:
        row = [
            float(test['requests_per_second']) / 1000,  # Convert to thousands
            float(test['avg_latency_ms']),
            float(test.get('p50_latency_ms', 0)),
            float(test.get('p90_latency_ms', 0)),
            float(test.get('p99_latency_ms', 0))
        ]
        values.append(row)
    
    # Normalize each column for better visualization
    values_array = np.array(values)
    normalized = np.zeros_like(values_array)
    for i in range(values_array.shape[1]):
        col = values_array[:, i]
        col_max = col.max()
        if col_max > 0:
            normalized[:, i] = col / col_max
    
    plt.figure(figsize=(10, max(8, len(tests) * 0.5)))
    im = plt.imshow(normalized, cmap='RdYlGn_r', aspect='auto')
    
    plt.xticks(range(len(metrics)), metrics)
    plt.yticks(range(len(tests)), tests)
    plt.xlabel('Metric', fontsize=12, fontweight='bold')
    plt.ylabel('Test Scenario', fontsize=12, fontweight='bold')
    plt.title('Lynx Performance Benchmarks - Normalized Heatmap', 
              fontsize=14, fontweight='bold')
    
    # Add colorbar
    cbar = plt.colorbar(im)
    cbar.set_label('Normalized Value (0-1)', rotation=270, labelpad=20)
    
    # Add text annotations with actual values
    for i in range(len(tests)):
        for j in range(len(metrics)):
            text = f'{values_array[i, j]:.1f}'
            if j == 0:  # RPS in thousands
                text += 'k'
            else:  # Latency in ms
                text += 'ms'
            plt.text(j, i, text, ha='center', va='center', 
                    color='white' if normalized[i, j] > 0.5 else 'black',
                    fontsize=8)
    
    plt.tight_layout()
    
    output_path = output_dir / 'performance_heatmap.png'
    plt.savefig(output_path, dpi=150, bbox_inches='tight')
    print(f"✓ Saved: {output_path}")
    plt.close()


def generate_summary_report(data, output_dir):
    """Generate a text summary report."""
    output_path = output_dir / 'summary.txt'
    
    with open(output_path, 'w') as f:
        f.write("=" * 70 + "\n")
        f.write("LYNX PERFORMANCE BENCHMARK SUMMARY\n")
        f.write("=" * 70 + "\n")
        f.write(f"Timestamp: {data['timestamp']}\n")
        f.write(f"API URL: {data['api_url']}\n")
        f.write(f"Redirect URL: {data['redirect_url']}\n")
        f.write("=" * 70 + "\n\n")
        
        # Find best and worst performers
        tests = data['tests']
        if tests:
            rps_values = [(t['name'], float(t['requests_per_second'])) for t in tests]
            rps_values.sort(key=lambda x: x[1], reverse=True)
            
            f.write("TOP PERFORMERS (by RPS):\n")
            f.write("-" * 70 + "\n")
            for i, (name, rps) in enumerate(rps_values[:3], 1):
                f.write(f"{i}. {name}\n")
                f.write(f"   {rps:,.0f} requests/second\n\n")
            
            f.write("\nDETAILED RESULTS:\n")
            f.write("-" * 70 + "\n")
            for test in tests:
                f.write(f"\n{test['name']}:\n")
                f.write(f"  RPS:         {float(test['requests_per_second']):,.0f}\n")
                f.write(f"  Avg Latency: {test['avg_latency_ms']} ms\n")
                if 'p50_latency_ms' in test and test['p50_latency_ms']:
                    f.write(f"  p50:         {test['p50_latency_ms']} ms\n")
                    f.write(f"  p90:         {test['p90_latency_ms']} ms\n")
                    f.write(f"  p99:         {test['p99_latency_ms']} ms\n")
                f.write(f"  Errors:      {test['errors']}\n")
        
        f.write("\n" + "=" * 70 + "\n")
    
    print(f"✓ Saved: {output_path}")


def main():
    parser = argparse.ArgumentParser(
        description='Generate visualizations from Lynx benchmark results'
    )
    parser.add_argument('json_file', help='Path to benchmark results JSON file')
    parser.add_argument('-o', '--output', default='./graphs',
                       help='Output directory for graphs (default: ./graphs)')
    
    args = parser.parse_args()
    
    json_path = Path(args.json_file)
    if not json_path.exists():
        print(f"Error: File not found: {json_path}")
        sys.exit(1)
    
    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)
    
    print(f"Loading benchmark results from: {json_path}")
    data = load_results(json_path)
    
    if not data.get('tests'):
        print("Error: No test results found in JSON file")
        sys.exit(1)
    
    print(f"Found {len(data['tests'])} test results")
    print(f"Generating visualizations in: {output_dir}")
    print()
    
    # Generate all visualizations
    plot_rps_comparison(data, output_dir)
    plot_latency_percentiles(data, output_dir)
    plot_performance_heatmap(data, output_dir)
    generate_summary_report(data, output_dir)
    
    print()
    print("=" * 70)
    print("✓ All visualizations generated successfully!")
    print("=" * 70)
    print(f"\nView results in: {output_dir}")


if __name__ == '__main__':
    main()
