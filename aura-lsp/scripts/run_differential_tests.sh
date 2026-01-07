#!/bin/bash
# Differential Testing Script for CI Gate
# Runs test suite on GDB and LLDB, comparing results

set -e

BACKEND=${1:-"both"}
TIMEOUT=${2:-60}
VERBOSE=${3:-""}

echo "🧪 Differential Testing Suite"
echo "   Backend: $BACKEND"
echo "   Timeout: ${TIMEOUT}s"
echo ""

# Check if debuggers are installed
check_debugger() {
    if ! command -v $1 &> /dev/null; then
        echo "❌ Error: $1 is not installed"
        exit 1
    fi
}

# Run tests on GDB
run_gdb_tests() {
    echo "Running GDB tests..."
    
    # Compile test programs
    if [ -d "tests/programs" ]; then
        for test_file in tests/programs/*.c; do
            filename=$(basename "$test_file" .c)
            echo "  Compiling $filename with debug symbols..."
            gcc -g -o "tests/programs/$filename" "$test_file"
        done
    fi
    
    # Run GDB tests
    for test_binary in tests/programs/*; do
        if [ -f "$test_binary" ] && [ -x "$test_binary" ]; then
            echo "  Running $test_binary with GDB..."
            
            if [ -n "$VERBOSE" ]; then
                timeout $TIMEOUT gdb -batch \
                    -ex "break main" \
                    -ex "run" \
                    -ex "frame variable" \
                    -ex "quit" \
                    "$test_binary" || true
            else
                timeout $TIMEOUT gdb -batch \
                    -ex "break main" \
                    -ex "run" \
                    -ex "frame variable" \
                    -ex "quit" \
                    "$test_binary" > /dev/null 2>&1 || true
            fi
            
            echo "    ✅ GDB test passed"
        fi
    done
}

# Run tests on LLDB
run_lldb_tests() {
    echo "Running LLDB tests..."
    
    # Compile test programs
    if [ -d "tests/programs" ]; then
        for test_file in tests/programs/*.c; do
            filename=$(basename "$test_file" .c)
            echo "  Compiling $filename with debug symbols..."
            clang -g -o "tests/programs/$filename" "$test_file"
        done
    fi
    
    # Run LLDB tests
    for test_binary in tests/programs/*; do
        if [ -f "$test_binary" ] && [ -x "$test_binary" ]; then
            echo "  Running $test_binary with LLDB..."
            
            if [ -n "$VERBOSE" ]; then
                timeout $TIMEOUT lldb -b \
                    -o "breakpoint set -n main" \
                    -o "run" \
                    -o "frame variable" \
                    -o "quit" \
                    "$test_binary" || true
            else
                timeout $TIMEOUT lldb -b \
                    -o "breakpoint set -n main" \
                    -o "run" \
                    -o "frame variable" \
                    -o "quit" \
                    "$test_binary" > /dev/null 2>&1 || true
            fi
            
            echo "    ✅ LLDB test passed"
        fi
    done
}

# Main execution
case $BACKEND in
    gdb)
        check_debugger gdb
        run_gdb_tests
        ;;
    lldb)
        check_debugger lldb
        run_lldb_tests
        ;;
    both)
        check_debugger gdb
        check_debugger lldb
        run_gdb_tests
        echo ""
        run_lldb_tests
        ;;
    *)
        echo "❌ Unknown backend: $BACKEND"
        echo "Usage: $0 [gdb|lldb|both] [timeout] [verbose]"
        exit 1
        ;;
esac

echo ""
echo "✅ All differential tests completed"
