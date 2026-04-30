#!/bin/bash
# Taiyang 插件快速部署脚本（macOS版）- VST3 + CLAP

PLUGIN_NAME="Taiyang"
VST3_DIR="$HOME/Library/Audio/Plug-Ins/VST3"
CLAP_DIR="$HOME/Library/Audio/Plug-Ins/CLAP"
VST3_BUNDLE="$VST3_DIR/${PLUGIN_NAME}.vst3"
CLAP_BUNDLE="$CLAP_DIR/${PLUGIN_NAME}.clap"

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

# === VST3 部署 ===
echo -e "${YELLOW}部署 VST3...${NC}"
mkdir -p "$VST3_BUNDLE/Contents/MacOS"
cp "target/release/lib${PLUGIN_NAME}.dylib" "$VST3_BUNDLE/Contents/MacOS/${PLUGIN_NAME}"
if [ $? -ne 0 ]; then
    echo -e "${RED}VST3 复制失败${NC}"
    exit 1
fi

if [ ! -f "$VST3_BUNDLE/Contents/Info.plist" ]; then
    cat > "$VST3_BUNDLE/Contents/Info.plist" << 'PLIST'
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
fi

codesign --force --sign - --deep "$VST3_BUNDLE"
echo -e "${GREEN}VST3 部署完成: $VST3_BUNDLE${NC}"

# === CLAP 部署 ===
echo -e "${YELLOW}部署 CLAP...${NC}"
# macOS CLAP 必须是 bundle 目录，结构类似 VST3
CLAP_BUNDLE="$CLAP_DIR/${PLUGIN_NAME}.clap"
mkdir -p "$CLAP_BUNDLE/Contents/MacOS"
cp "target/release/lib${PLUGIN_NAME}.dylib" "$CLAP_BUNDLE/Contents/MacOS/${PLUGIN_NAME}"
if [ $? -ne 0 ]; then
    echo -e "${RED}CLAP 复制失败${NC}"
    exit 1
fi

# CLAP bundle 也需要 Info.plist
if [ ! -f "$CLAP_BUNDLE/Contents/Info.plist" ]; then
    cat > "$CLAP_BUNDLE/Contents/Info.plist" << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleExecutable</key>
    <string>Taiyang</string>
    <key>CFBundleIdentifier</key>
    <string>com.jieneng.taiyang.clap</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>Taiyang</string>
    <key>CFBundlePackageType</key>
    <string>BNDL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>CSResourcesFileMapped</key>
    <true/>
</dict>
</plist>
PLIST
fi

# PkgInfo 是 macOS bundle 必需的
echo -n "BNDL????" > "$CLAP_BUNDLE/Contents/PkgInfo"

codesign --force --sign - --deep "$CLAP_BUNDLE"
echo -e "${GREEN}CLAP 部署完成: $CLAP_BUNDLE${NC}"

echo -e "${GREEN}全部部署完成！请在 DAW 中重新扫描插件。${NC}"
echo -e "${YELLOW}提示: 在 DAW 中执行 'Reset Plugin Catalog' 或 'Scan Now'${NC}"
