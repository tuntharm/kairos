#include "GatewayClient.h"

#include <Preferences.h>
#include <cstring>

namespace kairos {

namespace {

bool valid_hex_secret(const String& secret) {
  if (secret.length() != 64) {
    return false;
  }
  for (std::size_t index = 0; index < secret.length(); ++index) {
    const char ch = secret[index];
    const bool hex = (ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'f') ||
                     (ch >= 'A' && ch <= 'F');
    if (!hex) {
      return false;
    }
  }
  return true;
}

}  // namespace

GatewayClient::GatewayClient(AppShell& app, PomodoroModule& pomodoro)
    : app_(app), pomodoro_(pomodoro) {}

void GatewayClient::begin() {
  security_ready_ = run_security_self_test();
  if (!security_ready_) {
    app_.show_status("SECURITY SELF-TEST FAILED // USB ONLY",
                     StatusTone::Offline, millis(), true);
  }
}

void GatewayClient::loop(std::uint32_t now_ms) {
  (void)now_ms;
  poll_serial();
}

bool GatewayClient::security_ready() const { return security_ready_; }

void GatewayClient::send_listen_requested() {
  Serial.println("KAIROS_LISTEN requested");
}

bool GatewayClient::run_security_self_test() {
  Preferences preferences;
  if (!preferences.begin("kairos-sec", false)) {
    return false;
  }
  preferences.end();
  return true;
}

void GatewayClient::poll_serial() {
  while (Serial.available() > 0) {
    const char ch = static_cast<char>(Serial.read());
    if (ch == '\r') {
      continue;
    }
    if (ch != '\n') {
      if (serial_line_.length() < 160) {
        serial_line_ += ch;
      }
      continue;
    }
    serial_line_.trim();
    if (serial_line_.startsWith("KAIROS PROVISION ")) {
      const int first = serial_line_.indexOf(' ', 16);
      const String device = serial_line_.substring(16, first);
      const String secret = serial_line_.substring(first + 1);
      if (device.length() == 0 || !valid_hex_secret(secret)) {
        Serial.println("KAIROS_PROVISION rejected");
      } else {
        Preferences preferences;
        if (preferences.begin("kairos-sec", false) &&
            preferences.putString("device-id", device) > 0 &&
            preferences.putString("secret", secret) == 64) {
          Serial.println("KAIROS_PROVISION stored");
        } else {
          Serial.println("KAIROS_PROVISION store failed");
        }
        preferences.end();
      }
    } else if (serial_line_.startsWith("KAIROS GATEWAY ")) {
      Serial.println("KAIROS_GATEWAY noted; LAN client stays fail-closed here");
    } else if (serial_line_ == "KAIROS STATUS") {
      Serial.printf("KAIROS_STATUS security=%s sessions=%lu\n",
                    security_ready_ ? "ready" : "blocked",
                    static_cast<unsigned long>(pomodoro_.character_xp()));
    }
    serial_line_ = String();
  }
}

}  // namespace kairos
