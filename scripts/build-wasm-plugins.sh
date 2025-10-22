#!/bin/bash
set -e

echo "🔧 Building WASM Plugins for Local Development"
echo "=============================================="
echo ""

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Build all WASM plugins
echo -e "${BLUE}📦 Building WASM plugins...${NC}"
echo ""

# JS WASM Plugin
echo "Building js-wasm plugin..."
cargo build --target wasm32-wasip2 --release -p conveyor-plugin-js-wasm
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ js-wasm plugin built${NC}"
    ls -lh target/wasm32-wasip2/release/conveyor_plugin_js_wasm.wasm
else
    echo "❌ js-wasm plugin build failed"
    exit 1
fi

echo ""

# Echo WASM Plugin (optional)
echo "Building echo-wasm plugin (test plugin)..."
cargo build --target wasm32-wasip2 --release -p conveyor-plugin-echo-wasm
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ echo-wasm plugin built${NC}"
    ls -lh target/wasm32-wasip2/release/conveyor_plugin_echo_wasm.wasm
else
    echo "❌ echo-wasm plugin build failed"
    exit 1
fi

echo ""

# Excel WASM Plugin (optional)
echo "Building excel-wasm plugin..."
cargo build --target wasm32-wasip2 --release -p conveyor-plugin-excel-wasm
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ excel-wasm plugin built${NC}"
    ls -lh target/wasm32-wasip2/release/conveyor_plugin_excel_wasm.wasm
else
    echo "❌ excel-wasm plugin build failed"
    exit 1
fi

echo ""
echo -e "${GREEN}✅ All WASM plugins built successfully!${NC}"
echo "=============================================="
echo ""
echo "📍 Plugin location: target/wasm32-wasip2/release/"
echo ""
echo "💡 Usage in pipeline.toml:"
echo ""
echo "  [global]"
echo "  wasm_plugins = [\"js_wasm\"]"
echo ""
echo "  [[stages]]"
echo "  function = \"js.eval\""
echo "  [stages.config]"
echo "  script = '''function transform(row) { return row; }'''"
echo ""
