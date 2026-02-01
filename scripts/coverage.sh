#!/bin/bash
# Run code coverage locally using cargo-tarpaulin
# Usage: ./scripts/coverage.sh [--open]

set -e

echo "🔍 Running code coverage with cargo-tarpaulin..."

# Check if cargo-tarpaulin is installed
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo "⚠️  cargo-tarpaulin not found. Installing..."
    cargo install cargo-tarpaulin
fi

# Run coverage with configuration
echo "📊 Generating coverage report..."
cargo tarpaulin --config-file tarpaulin.toml --verbose "$@"

# Check if HTML report was generated
if [ -f "tarpaulin-report.html" ]; then
    echo ""
    echo "✅ Coverage report generated:"
    echo "   - XML: tarpaulin-report.xml"
    echo "   - HTML: tarpaulin-report.html"
    echo "   - LCOV: lcov.info"
    echo ""
    
    # Extract and display coverage percentage
    if command -v grep &> /dev/null && command -v bc &> /dev/null; then
        COVERAGE=$(grep -o 'line-rate="[0-9.]*"' tarpaulin-report.xml 2>/dev/null | head -1 | cut -d'"' -f2 || echo "0")
        if [ -n "$COVERAGE" ] && [ "$COVERAGE" != "0" ]; then
            COVERAGE_PCT=$(echo "$COVERAGE * 100" | bc)
            echo "📈 Line coverage: ${COVERAGE_PCT}%"
            
            # Check threshold
            if (( $(echo "$COVERAGE_PCT < 70.0" | bc -l) )); then
                echo "⚠️  WARNING: Coverage ${COVERAGE_PCT}% is below threshold of 70%"
                exit 1
            else
                echo "✅ Coverage meets threshold of 70%"
            fi
        fi
    fi
    
    # Open HTML report if --open flag is provided
    if [ "$1" == "--open" ]; then
        echo "🌐 Opening HTML report..."
        if command -v open &> /dev/null; then
            open tarpaulin-report.html
        elif command -v xdg-open &> /dev/null; then
            xdg-open tarpaulin-report.html
        else
            echo "   Please open tarpaulin-report.html manually"
        fi
    fi
else
    echo "❌ Coverage report generation failed"
    exit 1
fi

echo ""
echo "💡 Tip: Run with --open flag to automatically open the HTML report"
echo "   ./scripts/coverage.sh --open"
