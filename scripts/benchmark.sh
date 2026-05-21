#!/usr/bin/env bash
#
# jv Benchmark Runner
# Structured performance comparison between jv and Maven/Gradle.
#
# Usage:
#   ./scripts/benchmark.sh <project-path> [options]
#
# Examples:
#   ./scripts/benchmark.sh /path/to/spring-boot-app
#   ./scripts/benchmark.sh . --mode cold
#   ./scripts/benchmark.sh . --mode no-cache
#

set -euo pipefail

PROJECT_PATH="${1:-.}"
MODE="${2:-warm}"
MAX_AGE_DAYS=90

# Create results directory if it doesn't exist
RESULTS_DIR="$(dirname "$0")/../results"
mkdir -p "$RESULTS_DIR"

# Generate timestamped result file
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
PROJECT_NAME=$(basename "$PROJECT_PATH")
RESULT_FILE="$RESULTS_DIR/${PROJECT_NAME}_${TIMESTAMP}.txt"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${BLUE}============================================================${NC}"
    echo -e "${BLUE}  jv Benchmark Runner${NC}"
    echo -e "${BLUE}============================================================${NC}"
}

print_section() {
    echo -e "\n${YELLOW}▶ $1${NC}"
}

log() {
    echo -e "${GREEN}[INFO]${NC} $1" | tee -a "$RESULT_FILE"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1" | tee -a "$RESULT_FILE"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1" | tee -a "$RESULT_FILE"
}

# Also capture raw output
echo "=== jv Benchmark Run: $(date) ===" > "$RESULT_FILE"

# Resolve absolute path
PROJECT_PATH=$(cd "$PROJECT_PATH" && pwd)

if [[ ! -d "$PROJECT_PATH" ]]; then
    error "Project path does not exist: $PROJECT_PATH"
    exit 1
fi

# Detect build system
if [[ -f "$PROJECT_PATH/pom.xml" ]]; then
    BUILD_SYSTEM="maven"
elif [[ -f "$PROJECT_PATH/build.gradle" ]] || [[ -f "$PROJECT_PATH/build.gradle.kts" ]]; then
    BUILD_SYSTEM="gradle"
else
    error "No supported build file found (pom.xml, build.gradle, build.gradle.kts)"
    exit 1
fi

log "Project: $PROJECT_PATH"
log "Build system: $BUILD_SYSTEM"
log "Mode: $MODE"

# Capture environment
DATE=$(date '+%Y-%m-%d %H:%M:%S')
OS=$(uname -s)
ARCH=$(uname -m)
JAVA_VERSION=$(java -version 2>&1 | head -1 | sed 's/.*version "\(.*\)"./\1/')
MAVEN_VERSION=$(mvn -version 2>/dev/null | head -1 || echo "Maven not found")
JV_VERSION=$(git -C "$(dirname "$0")/.." rev-parse --short HEAD 2>/dev/null || echo "unknown")

print_section "Environment"
echo "Date:             $DATE"
echo "OS / Arch:        $OS / $ARCH"
echo "Java:             $JAVA_VERSION"
echo "Maven:            $MAVEN_VERSION"
echo "jv (git):         $JV_VERSION"

# --- Maven Benchmark ---
run_maven_benchmark() {
    print_section "Running Maven benchmark"

    # Try to locate Maven (especially on macOS + Homebrew)
    MVN_CMD=$(command -v mvn 2>/dev/null || command -v /opt/homebrew/bin/mvn 2>/dev/null || echo "")
    if [[ -z "$MVN_CMD" ]]; then
        warn "Maven (mvn) not found in PATH or common Homebrew location — skipping Maven benchmark"
        return 0
    fi

    local start end duration

    # Clean Maven local cache for cold runs
    if [[ "$MODE" == "cold" ]]; then
        log "Cleaning Maven local repository (~/.m2/repository)..."
        rm -rf ~/.m2/repository/* 2>/dev/null || true
    fi

    start=$(date +%s.%N)

    (
        cd "$PROJECT_PATH"
        "$MVN_CMD" dependency:go-offline -B -q
    )

    end=$(date +%s.%N)
    duration=$(echo "$end - $start" | bc)

    echo ""
    log "Maven dependency:go-offline completed in ${duration}s"
}

# --- jv Benchmark ---
run_jv_benchmark() {
    print_section "Running jv benchmark"

    local jv_bin
    local start end duration
    local artifacts="unknown"

    # Try release binary first, fall back to cargo run
    if [[ -f "$(dirname "$0")/../target/release/jv" ]]; then
        jv_bin="$(dirname "$0")/../target/release/jv"
    else
        jv_bin="cargo run --quiet --release --"
    fi

    local jv_cmd="$jv_bin resolve $PROJECT_PATH --output /tmp/jv-bench.lock"

    if [[ "$MODE" == "no-cache" ]]; then
        jv_cmd="$jv_cmd --no-cache"
    fi

    # Clean jv cache for cold runs
    if [[ "$MODE" == "cold" ]]; then
        log "Cleaning jv cache..."
        "$jv_bin" cache clean 2>/dev/null || true
    fi

    start=$(date +%s.%N)

    # Capture output to parse artifact count
    local jv_output
    if jv_output=$(eval "$jv_cmd" 2>&1); then
        echo "$jv_output"
        end=$(date +%s.%N)
        duration=$(echo "$end - $start" | bc)

        # Extract artifact count (handles both "Resolved X artifacts" and the new log format)
        artifacts=$(echo "$jv_output" | grep -oE "(Resolved [0-9]+ artifacts|Transitive resolution complete: [0-9]+ artifacts)" | grep -oE "[0-9]+" | head -1 || echo "unknown")

        echo ""
        log "jv resolve completed in ${duration}s"
        log "Artifacts resolved: $artifacts"
    else
        error "jv resolve failed"
        return 1
    fi
}

# Main execution
print_header

case "$MODE" in
    cold|warm|no-cache)
        if [[ "$BUILD_SYSTEM" == "maven" ]]; then
            run_maven_benchmark || true
        fi
        run_jv_benchmark || true
        ;;
    *)
        error "Unknown mode: $MODE (supported: cold, warm, no-cache)"
        exit 1
        ;;
esac

echo ""
log "Benchmark run complete."
log "Full results saved to: $RESULT_FILE"
echo ""
echo "You can copy relevant parts into BENCHMARKS.md for long-term tracking."
