#!/usr/bin/env sh
set -eu

OUT_DIR="${1:-target/dev-tls}"
CERT_FILE="${OUT_DIR}/sysmaster-dev.crt"
KEY_FILE="${OUT_DIR}/sysmaster-dev.key"
DAYS="${DEV_TLS_DAYS:-365}"

mkdir -p "${OUT_DIR}"

openssl req \
  -x509 \
  -newkey rsa:4096 \
  -sha256 \
  -days "${DAYS}" \
  -nodes \
  -keyout "${KEY_FILE}" \
  -out "${CERT_FILE}" \
  -subj "/CN=localhost" \
  -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"

chmod 600 "${KEY_FILE}"
chmod 644 "${CERT_FILE}"

cat <<EOF
Generated development TLS certificate material:
  Certificate: ${CERT_FILE}
  Private key: ${KEY_FILE}

Use one of these approaches:
  1. Copy them to the paths configured by sysmaster:
     api.tls.cert-file: etc/web/api.crt
     api.tls.key-file: etc/web/api.key

  2. Point the config directly at the generated files:
     api.tls.enabled: true
     api.tls.cert-file: ${CERT_FILE}
     api.tls.key-file: ${KEY_FILE}
     api.tls.allow-insecure: true

Swagger UI will then be available over HTTPS at:
  https://localhost:4202/doc/
EOF
