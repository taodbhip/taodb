#!/usr/bin/env bash
set -eu

# ── TaoDB 一键安装脚本 ──
# curl -fsSL https://raw.githubusercontent.com/taodbhip/taodb/main/install.sh | bash
#
# 可选参数:
#   --version v1.0.0   安装指定版本（默认最新）
#   --prefix /usr/local 安装到指定前缀（默认 /usr/local/bin 或 ~/.local/bin）
#   --help              显示帮助

REPO="taodbhip/taodb"
DEFAULT_PREFIX="/usr/local"

# ── 颜色 ──
BOLD="$(tput bold 2>/dev/null || echo '')"
GREEN="$(tput setaf 2 2>/dev/null || echo '')"
YELLOW="$(tput setaf 3 2>/dev/null || echo '')"
RED="$(tput setaf 1 2>/dev/null || echo '')"
RESET="$(tput sgr0 2>/dev/null || echo '')"

info()  { echo "${BOLD}${GREEN}→${RESET} $*"; }
warn()  { echo "${YELLOW}⚠${RESET} $*"; }
error() { echo "${RED}✗${RESET} $*"; exit 1; }

# ── 参数解析 ──
VERSION=""
PREFIX=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) VERSION="$2"; shift 2 ;;
    --prefix)  PREFIX="$2";  shift 2 ;;
    --help)    cat <<'HELP'
TaoDB 一键安装脚本

用法:
  curl -fsSL https://raw.githubusercontent.com/taodbhip/taodb/main/install.sh | bash
  bash install.sh [--version v1.0.0] [--prefix /usr/local]

可选参数:
  --version v1.0.0   安装指定版本（默认最新）
  --prefix /usr/local 安装到指定前缀（默认 /usr/local/bin 或 ~/.local/bin）
  --help              显示帮助
HELP
  exit 0 ;;
    *) error "未知参数: $1" ;;
  esac
done

# ── 平台检测 ──
detect_target() {
  local os arch
  case "$(uname -s)" in
    Darwin)  os="apple-darwin" ;;
    Linux)   os="unknown-linux-gnu" ;;
    *)       error "不支持的操作系统: $(uname -s)" ;;
  esac
  case "$(uname -m)" in
    arm64|aarch64) arch="aarch64" ;;
    x86_64|amd64)  arch="x86_64" ;;
    *)             error "不支持的架构: $(uname -m)" ;;
  esac
  echo "${arch}-${os}"
}

TARGET="$(detect_target)"
info "检测到平台: ${TARGET}"

# ── 获取版本号 ──
if [[ -z "$VERSION" ]]; then
  info "查询最新版本..."
  VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name":' \
    | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')
  if [[ -z "$VERSION" ]]; then
    error "无法获取最新版本号，请用 --version 指定"
  fi
fi
info "版本: ${VERSION}"

# ── 安装目录 ──
if [[ -z "$PREFIX" ]]; then
  if [[ -w /usr/local/bin ]]; then
    PREFIX="/usr/local"
  else
    PREFIX="${HOME}/.local"
    mkdir -p "${PREFIX}/bin"
  fi
fi
BIN_DIR="${PREFIX}/bin"
mkdir -p "$BIN_DIR"

if [[ ":$PATH:" != *":${BIN_DIR}:"* ]]; then
  warn "${BIN_DIR} 不在 PATH 中"
  if [[ "$SHELL" == *"zsh"* ]]; then
    echo "  echo 'export PATH=\"${BIN_DIR}:\$PATH\"' >> ~/.zshrc && source ~/.zshrc"
  else
    echo "  echo 'export PATH=\"${BIN_DIR}:\$PATH\"' >> ~/.bashrc && source ~/.bashrc"
  fi
fi

# ── 下载 ──
ARCHIVE="taodb-${VERSION}-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

info "下载 ${ARCHIVE}..."
if command -v curl &>/dev/null; then
  curl -fsSL "$URL" -o "${TMPDIR}/${ARCHIVE}"
else
  wget -q "$URL" -O "${TMPDIR}/${ARCHIVE}"
fi

# ── 校验和（如果有 checksums.txt） ──
CHECKSUMS_URL="https://github.com/${REPO}/releases/download/${VERSION}/checksums.txt"
if curl -fsSLI "$CHECKSUMS_URL" &>/dev/null; then
  info "验证校验和..."
  curl -fsSL "$CHECKSUMS_URL" -o "${TMPDIR}/checksums.txt"
  EXPECTED=$(grep "$ARCHIVE" "${TMPDIR}/checksums.txt" | awk '{print $1}')
  if [[ -n "$EXPECTED" ]]; then
    ACTUAL=$(sha256sum "${TMPDIR}/${ARCHIVE}" | awk '{print $1}')
    if [[ "$EXPECTED" != "$ACTUAL" ]]; then
      error "校验和不匹配！\n  期望: ${EXPECTED}\n  实际: ${ACTUAL}"
    fi
  fi
fi

# ── 解压安装 ──
info "安装到 ${BIN_DIR}/taodb"
tar xzf "${TMPDIR}/${ARCHIVE}" -C "$TMPDIR"
install -m 755 "${TMPDIR}/taodb" "${BIN_DIR}/taodb"

# ── macOS Gatekeeper 提示 ──
if [[ "$(uname -s)" == "Darwin" ]]; then
  echo ""
  warn "macOS 首次运行可能需要允许:"
  echo "  ${BOLD}右键点击${RESET} ${BIN_DIR}/taodb → 打开"
  echo "  或运行: xattr -d com.apple.quarantine ${BIN_DIR}/taodb"
fi

# ── 验证 ──
echo ""
if "${BIN_DIR}/taodb" --help &>/dev/null; then
  VER=$("${BIN_DIR}/taodb" --version 2>/dev/null || echo "$VERSION")
  info "安装完成 — ${GREEN}${VER}${RESET}"
  echo ""
  echo "开始使用:"
  echo "  cd your-project"
  echo "  taodb init"
else
  error "二进制验证失败"
fi
