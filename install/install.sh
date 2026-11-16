#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-3.0-or-later
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

DATA_DIR="/var/lib/opensim"
CONFIG_DIR="/etc/opensim"
BIN_DIR="/usr/local/bin"
SERVICE_FILE="/etc/systemd/system/welkin-robust.service"
PORTAL_PORT=8002
GRID_PORT=8003

info()  { printf "${BLUE}[INFO]${NC}  %s\n" "$*"; }
ok()    { printf "${GREEN}[OK]${NC}    %s\n" "$*"; }
warn()  { printf "${YELLOW}[WARN]${NC}  %s\n" "$*"; }
err()   { printf "${RED}[ERROR]${NC} %s\n" "$*" >&2; }

check_root() {
    if [[ $EUID -ne 0 ]]; then
        err "This script must be run as root"
        exit 1
    fi
}

detect_distro() {
    if [[ -f /etc/os-release ]]; then
        # shellcheck source=/dev/null
        . /etc/os-release
        case "${ID:-}" in
            fedora|rhel|centos|rocky|alma)
                DISTRO="fedora"
                PKG_MGR="dnf"
                ;;
            ubuntu|debian|linuxmint|pop)
                DISTRO="debian"
                PKG_MGR="apt-get"
                ;;
            arch|manjaro|endeavouros)
                DISTRO="arch"
                PKG_MGR="pacman"
                ;;
            *)
                DISTRO="unknown"
                PKG_MGR=""
                ;;
        esac
    else
        DISTRO="unknown"
        PKG_MGR=""
    fi
    info "Detected distribution: ${DISTRO}"
}

check_prerequisites() {
    local missing=()

    if ! command -v uuidgen &>/dev/null; then
        missing+=("uuid-runtime/util-linux")
    fi

    if [[ ${#missing[@]} -gt 0 ]]; then
        warn "Missing prerequisites: ${missing[*]}"
        case "${PKG_MGR}" in
            dnf)  dnf install -y util-linux ;;
            apt-get) apt-get update && apt-get install -y uuid-runtime ;;
            pacman) pacman -S --noconfirm util-linux ;;
            *) err "Cannot install prerequisites on unknown distro"; exit 1 ;;
        esac
    fi
    ok "Prerequisites satisfied"
}

create_user() {
    if ! id opensim &>/dev/null; then
        useradd --system --home-dir "$DATA_DIR" --shell /usr/sbin/nologin opensim
        ok "Created system user: opensim"
    else
        ok "User opensim already exists"
    fi
}

create_directories() {
    mkdir -p "$DATA_DIR"/{data,logs,backups}
    mkdir -p "$CONFIG_DIR"
    chown -R opensim:opensim "$DATA_DIR"
    ok "Data directories ready: $DATA_DIR/{data,logs,backups}"
}

generate_config() {
    local config_file="$CONFIG_DIR/robust.ini"
    if [[ -f "$config_file" ]]; then
        ok "Config already exists: $config_file"
        return
    fi

    cat > "$config_file" << 'CONF'
[Startup]
ConfigDirectory = /etc/opensim
DataDirectory = /var/lib/opensim/data
LogDirectory = /var/lib/opensim/logs

[Network]
HttpListenerPort = 8002
GridServicePort = 8003

[DatabaseService]
StorageProvider = sqlite
ConnectionString = Data Source=/var/lib/opensim/data/opensim.db

[GridService]
Region_Welcome = DefaultRegion

[AuthenticationService]
AllowGetAuthInfo = false
AllowSetAuthInfo = false
AllowSetPassword = false

[Portal]
Enabled = true
CONF

    chown opensim:opensim "$config_file"
    chmod 640 "$config_file"
    ok "Generated config: $config_file"
}

generate_admin_token() {
    local token_file="$CONFIG_DIR/admin-token"
    if [[ -f "$token_file" ]]; then
        ok "Admin token already exists"
        return
    fi

    local token
    token="$(uuidgen)"
    printf '%s' "$token" > "$token_file"
    chown opensim:opensim "$token_file"
    chmod 600 "$token_file"
    ok "Generated admin token: $token_file"
}

setup_database() {
    local db_file="$DATA_DIR/data/opensim.db"
    if [[ -f "$db_file" ]]; then
        ok "Database already exists: $db_file"
        return
    fi

    touch "$db_file"
    chown opensim:opensim "$db_file"

    if command -v "$BIN_DIR/welkin-robust" &>/dev/null; then
        info "Running database migrations..."
        sudo -u opensim "$BIN_DIR/welkin-robust" --config "$CONFIG_DIR/robust.ini" --migrate || true
    else
        warn "welkin-robust binary not found; migrations will run on first start"
    fi
    ok "Database ready: $db_file"
}

install_service() {
    local script_dir
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    local source_service="$script_dir/welkin-robust.service"

    if [[ -f "$source_service" ]]; then
        cp "$source_service" "$SERVICE_FILE"
    else
        cat > "$SERVICE_FILE" << 'UNIT'
[Unit]
Description=OpenSimulator Grid Services
After=network.target

[Service]
Type=simple
User=opensim
Group=opensim
ExecStart=/usr/local/bin/welkin-robust --config /etc/opensim/robust.ini
WorkingDirectory=/var/lib/opensim
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
UNIT
    fi

    systemctl daemon-reload
    systemctl enable welkin-robust.service
    ok "Systemd service installed and enabled"
}

start_service() {
    if systemctl is-active --quiet welkin-robust.service; then
        info "Service already running; restarting..."
        systemctl restart welkin-robust.service
    else
        systemctl start welkin-robust.service
    fi

    sleep 2
    if systemctl is-active --quiet welkin-robust.service; then
        ok "Service started"
    else
        warn "Service may not have started; check: journalctl -u welkin-robust"
    fi
}

print_summary() {
    local ip
    ip="$(hostname -I 2>/dev/null | awk '{print $1}')"
    ip="${ip:-localhost}"

    echo ""
    printf "${GREEN}════════════════════════════════════════════════════${NC}\n"
    printf "${GREEN}  OpenSimulator installed successfully${NC}\n"
    printf "${GREEN}════════════════════════════════════════════════════${NC}\n"
    echo ""
    info "Portal:       http://${ip}:${PORTAL_PORT}/"
    info "Grid services: http://${ip}:${GRID_PORT}/"
    info "Config:       $CONFIG_DIR/robust.ini"
    info "Data:         $DATA_DIR/"
    info "Logs:         journalctl -u welkin-robust"
    info "Admin token:  $CONFIG_DIR/admin-token"
    echo ""
}

main() {
    echo ""
    info "OpenSimulator Installer"
    echo ""

    check_root
    detect_distro
    check_prerequisites
    create_user
    create_directories
    generate_config
    generate_admin_token
    setup_database
    install_service
    start_service
    print_summary
}

main "$@"
