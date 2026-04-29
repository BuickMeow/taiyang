#!/bin/bash
# Taiyang VST3 插件快速部署脚本（macOS版）

PLUGIN_NAME="Taiyang"
VST3_DIR="$HOME/Library/Audio/Plug-Ins/VST3"
BUNDLE_DIR="$VST3_DIR/${PLUGIN_NAME}.vst3"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}开始构建 ${PLUGIN_NAME}...${NC}"

# 1. 编译
cargo build --release
if [ $? -ne 0 ]; then
    echo -e "${RED}编译失败${NC}"
    exit 1
fi

echo -e "${GREEN}编译成功${NC}"

# 2. 确保目录存在
mkdir -p "$BUNDLE_DIR/Contents/MacOS"

# 3. 复制文件（覆盖旧版本）
cp "target/release/lib${PLUGIN_NAME}.dylib" "$BUNDLE_DIR/Contents/MacOS/${PLUGIN_NAME}"
if [ $? -ne 0 ]; then
    echo -e "${RED}复制失败${NC}"
    exit 1
fi

echo -e "${GREEN}部署到: $BUNDLE_DIR${NC}"

# 4. 检查 Info.plist 是否存在
if [ ! -f "$BUNDLE_DIR/Contents/Info.plist" ]; then
    echo -e "${YELLOW}Info.plist 不存在，创建中...${NC}"
    cat > "$BUNDLE_DIR/Contents/Info.plist" << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleExecutable</key>
    <string>Taiyang</string>
    <key>CFBundleIdentifier</key>
    <string>com.jieneng.taiyang</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>Taiyang</string>
    <key>CFBundlePackageType</key>
    <string>BNDL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST
    echo -e "${GREEN}Info.plist 创建完成${NC}"
fi

echo "签名插件..."
codesign --force --sign - --deep "$VST3_DIR/$PLUGIN_NAME.vst3"
echo "签名完成"

echo -e "${GREEN}部署完成！请在 DAW 中重新扫描插件。${NC}"
echo -e "${YELLOW}提示: 在 DAW 中执行 'Reset Plugin Catalog' 或 'Scan Now'${NC}"
