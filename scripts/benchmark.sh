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
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

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

    local start end duration

    # Clean Maven local cache for cold runs
    if [[ "$MODE" == "cold" ]]; then
        log "Cleaning Maven local repository (~/.m2/repository)..."
        rm -rf ~/.m2/repository/* 2>/dev/null || true
    fi

    start=$(date +%s.%N)

    if command -v mvn >/dev/null 2>&1; then
        (
            cd "$PROJECT_PATH"
            mvn dependency:go-offline -B -q
        )
    else
        error "Maven (mvn) not found in PATH"
        return 1
    fi

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

    if eval "$jv_cmd"; then
        end=$(date +%s.%N)
        duration=$(echo "$end - $start" | bc)
        echo ""
        log "jv resolve completed in ${duration}s"
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
echo "You can copy the output above into BENCHMARKS.md or results/ for tracking."
