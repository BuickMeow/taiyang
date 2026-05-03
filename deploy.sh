#!/bin/bash
# Taiyang 插件快速部署脚本（macOS版）- VST3 + CLAP
# 同时部署 taiyang（单通道）和 taiyang16（16通道）

VST3_DIR="$HOME/Library/Audio/Plug-Ins/VST3"
CLAP_DIR="$HOME/Library/Audio/Plug-Ins/CLAP"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}开始构建所有插件...${NC}"

# 1. 编译 workspace 全部 crate
cargo build --release
if [ $? -ne 0 ]; then
    echo -e "${RED}编译失败${NC}"
    exit 1
fi

echo -e "${GREEN}编译成功${NC}"

# 部署单个插件的函数
deploy_plugin() {
    local PLUGIN_NAME="$1"
    local BUNDLE_ID="$2"
    local LIB_NAME="lib${PLUGIN_NAME}.dylib"

    echo -e "${YELLOW}部署 ${PLUGIN_NAME}...${NC}"

    # === VST3 部署 ===
    local VST3_BUNDLE="$VST3_DIR/${PLUGIN_NAME}.vst3"
    mkdir -p "$VST3_BUNDLE/Contents/MacOS"
    cp "target/release/${LIB_NAME}" "$VST3_BUNDLE/Contents/MacOS/${PLUGIN_NAME}"
    if [ $? -ne 0 ]; then
        echo -e "${RED}${PLUGIN_NAME} VST3 复制失败${NC}"
        return 1
    fi

    if [ ! -f "$VST3_BUNDLE/Contents/Info.plist" ]; then
        cat > "$VST3_BUNDLE/Contents/Info.plist" << PLISTEOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleExecutable</key>
    <string>${PLUGIN_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>${PLUGIN_NAME}</string>
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
PLISTEOF
    fi

    codesign --force --sign - --deep "$VST3_BUNDLE"
    echo -e "${GREEN}  VST3 部署完成: $VST3_BUNDLE${NC}"

    # === CLAP 部署 ===
    local CLAP_BUNDLE="$CLAP_DIR/${PLUGIN_NAME}.clap"
    mkdir -p "$CLAP_BUNDLE/Contents/MacOS"
    cp "target/release/${LIB_NAME}" "$CLAP_BUNDLE/Contents/MacOS/${PLUGIN_NAME}"
    if [ $? -ne 0 ]; then
        echo -e "${RED}${PLUGIN_NAME} CLAP 复制失败${NC}"
        return 1
    fi

    if [ ! -f "$CLAP_BUNDLE/Contents/Info.plist" ]; then
        cat > "$CLAP_BUNDLE/Contents/Info.plist" << PLISTEOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleExecutable</key>
    <string>${PLUGIN_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}.clap</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>${PLUGIN_NAME}</string>
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
PLISTEOF
    fi

    echo -n "BNDL????" > "$CLAP_BUNDLE/Contents/PkgInfo"

    codesign --force --sign - --deep "$CLAP_BUNDLE"
    echo -e "${GREEN}  CLAP 部署完成: $CLAP_BUNDLE${NC}"
}

# 依次部署两个插件
deploy_plugin "taiyang" "com.jieneng.taiyang"
deploy_plugin "taiyang16" "com.jieneng.taiyang16"

echo -e "${GREEN}全部部署完成！请在 DAW 中重新扫描插件。${NC}"
echo -e "${YELLOW}提示: 在 DAW 中执行 'Reset Plugin Catalog' 或 'Scan Now'${NC}"
echo -e ""
echo -e "两个插件："
echo -e "  Taiyang   - 单通道 SoundFont 合成器"
echo -e "  Taiyang16 - 16通道 SoundFont 合成器（完整 MIDI 支持）"
