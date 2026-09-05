#include <Arduino.h>
#include <Preferences.h>
#include <TFT_Touch.h>
#include <TFT_eSPI.h>
#include <WiFi.h>
#include <WiFiManager.h>
#include <esp_system.h>

#include <cstdio>
#include <cstring>

#include "AppShell.h"
#include "GatewayClient.h"
#include "PomodoroModule.h"
#include "WardrobeModule.h"
#include "board/e32r28t.hpp"

namespace {

TFT_eSPI display;
TFT_Touch touch(kairos::board::kTouchChipSelect,
                kairos::board::kTouchClock,
                kairos::board::kTouchMosi,
                kairos::board::kTouchMiso);
WiFiManager wifi_manager;
kairos::PomodoroModule pomodoro;
kairos::WardrobeModule wardrobe;
kairos::AppModule* const modules[] = {&pomodoro, &wardrobe};
kairos::AppShell app(display, modules, sizeof(modules) / sizeof(modules[0]));
kairos::GatewayClient gateway(app, pomodoro);

bool touch_latched = false;
bool portal_started = false;
std::uint32_t portal_started_ms = 0;

constexpr char kPortalAlphabet[] =
    "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
constexpr std::size_t kPortalPasswordLength = 12;
constexpr std::uint32_t kPortalLifetimeMs = 180U * 1000U;

bool valid_portal_password(const String& password) {
  if (password.length() != kPortalPasswordLength) {
    return false;
  }
  for (std::size_t index = 0; index < password.length(); ++index) {
    if (std::strchr(kPortalAlphabet, password[index]) == nullptr) {
      return false;
    }
  }
  return true;
}

bool create_portal_password(String& password) {
  Preferences preferences;
  if (!preferences.begin("kairos-wifi", false)) {
    return false;
  }
  const bool cleanup_required = preferences.isKey("portal-dirty") &&
                                preferences.getBool("portal-dirty", false);
  const bool stored_password = preferences.isKey("portal-pass");
  if (stored_password && !preferences.remove("portal-pass")) {
    preferences.end();
    return false;
  }
  if (cleanup_required &&
      preferences.putBool("portal-dirty", false) != 1) {
    preferences.end();
    return false;
  }
  if (preferences.putBool("portal-dirty", true) != 1) {
    preferences.end();
    return false;
  }

  std::uint8_t random_bytes[kPortalPasswordLength];
  esp_fill_random(random_bytes, sizeof(random_bytes));
  char generated[kPortalPasswordLength + 1];
  for (std::size_t index = 0; index < kPortalPasswordLength; ++index) {
    generated[index] = kPortalAlphabet[random_bytes[index] & 31U];
  }
  generated[kPortalPasswordLength] = '\0';
  password = generated;
  if (!valid_portal_password(password) ||
      preferences.putString("portal-pass", password) !=
          kPortalPasswordLength) {
    preferences.end();
    password = String();
    return false;
  }
  preferences.end();
  return true;
}

bool retire_portal_password() {
  Preferences preferences;
  if (!preferences.begin("kairos-wifi", false)) {
    return false;
  }
  const bool removed = !preferences.isKey("portal-pass") ||
                       preferences.remove("portal-pass");
  const bool retired =
      removed && preferences.putBool("portal-dirty", false) == 1;
  preferences.end();
  return retired;
}

void hide_portal_credentials() {
  app.clear_status();
  const bool retired = retire_portal_password();
  if (retired) {
    Serial.println(
        "KAIROS_WIFI setup portal closed; password retired and hidden");
  } else {
    Serial.println(
        "KAIROS_WIFI password cleanup failed; next setup will fail closed "
        "unless storage recovers");
  }
  if (!gateway.security_ready()) {
    app.show_status("SECURITY SELF-TEST FAILED // USB ONLY",
                    kairos::StatusTone::Offline, millis(), true);
  } else if (!retired) {
    app.show_status("WIFI PASSWORD CLEANUP FAILED // SETUP LOCKED",
                    kairos::StatusTone::Offline, millis());
  }
}

void show_boot_sequence() {
  constexpr std::uint16_t background = 0x0000;
  constexpr std::uint16_t accent = 0x06DF;
  display.fillScreen(background);
  display.setTextDatum(MC_DATUM);
  display.setTextColor(accent, background);
  display.drawString("KAIROS", 160, 98, 4);
  display.drawString("CONSOLE // BOOT", 160, 129, 2);
  for (int width = 0; width <= 240; width += 24) {
    display.fillRect(40, 160, width, 3, accent);
    delay(25);
  }
  display.setTextDatum(TL_DATUM);
}

void request_listen() {
  gateway.send_listen_requested();
  digitalWrite(kairos::board::kLedBlue, LOW);
  delay(45);
  digitalWrite(kairos::board::kLedBlue, HIGH);
}

void start_wifi_if_needed() {
  WiFi.mode(WIFI_STA);
  wifi_manager.setConfigPortalBlocking(false);
  wifi_manager.setConfigPortalTimeout(180);
  wifi_manager.setAPClientCheck(false);
  wifi_manager.setWebPortalClientCheck(false);
  const bool force_portal = digitalRead(kairos::board::kBootButton) == LOW;
  if (force_portal || WiFi.SSID().isEmpty()) {
    String password;
    if (!create_portal_password(password)) {
      Serial.println(
          "KAIROS_WIFI secure setup unavailable; portal was not opened");
      if (!gateway.security_ready()) {
        app.show_status("SECURITY SELF-TEST FAILED // USB ONLY",
                        kairos::StatusTone::Offline, millis(), true);
      } else {
        app.show_status("WIFI SETUP ERROR // USB SERIAL FOR HELP",
                        kairos::StatusTone::Offline, millis());
      }
      return;
    }
    char ssid_buffer[32];
    std::snprintf(ssid_buffer, sizeof(ssid_buffer), "Kairos-Setup-%06lX",
                  static_cast<unsigned long>(ESP.getEfuseMac() & 0xffffffULL));
    const String ssid = ssid_buffer;
    wifi_manager.startConfigPortal(ssid.c_str(), password.c_str());
    portal_started = wifi_manager.getConfigPortalActive();
    if (portal_started) {
      portal_started_ms = millis();
      String setup_message = "SSID ";
      setup_message += ssid;
      setup_message += " PASS ";
      setup_message += password;
      app.show_status(setup_message, kairos::StatusTone::Info,
                      portal_started_ms, true);
      Serial.printf("KAIROS_WIFI secure setup portal %s; password is shown "
                    "only on the console\n",
                    ssid.c_str());
    } else {
      Serial.println("KAIROS_WIFI secure setup portal failed to start");
      retire_portal_password();
    }
    for (std::size_t index = 0; index < password.length(); ++index) {
      password.setCharAt(index, ' ');
    }
    password = String();
  } else {
    WiFi.begin();
  }
}

void poll_wifi_portal(std::uint32_t now_ms) {
  if (!portal_started) {
    return;
  }
  wifi_manager.process();
  if (wifi_manager.getConfigPortalActive() &&
      now_ms - portal_started_ms >= kPortalLifetimeMs) {
    wifi_manager.stopConfigPortal();
  }
  if (!wifi_manager.getConfigPortalActive()) {
    portal_started = false;
    hide_portal_credentials();
  }
}

void poll_touch(std::uint32_t now_ms) {
  const bool pressed = digitalRead(kairos::board::kTouchInterrupt) == LOW &&
                       touch.Pressed();
  if (!pressed) {
    touch_latched = false;
    return;
  }
  if (touch_latched) {
    return;
  }
  touch_latched = true;
  const std::uint16_t x = touch.X();
  const std::uint16_t y = touch.Y();
  if (x < kairos::board::kScreenWidth && y < kairos::board::kScreenHeight) {
    app.touch(kairos::TouchPoint{x, y}, now_ms);
  }
}

}  // namespace

void setup() {
  Serial.begin(115200);
  kairos::board::prepare_safe_outputs();

  display.init();
  display.setRotation(kairos::board::kLandscapeRotation);
  display.fillScreen(TFT_BLACK);
  digitalWrite(kairos::board::kDisplayBacklight, HIGH);

  touch.setResolution(kairos::board::kScreenWidth,
                      kairos::board::kScreenHeight);
  touch.setCal(kairos::board::kTouchXMinimum,
               kairos::board::kTouchXMaximum,
               kairos::board::kTouchYMinimum,
               kairos::board::kTouchYMaximum,
               kairos::board::kScreenWidth, kairos::board::kScreenHeight,
               kairos::board::kTouchAxisSwap);
  touch.setRotation(kairos::board::kLandscapeRotation);

  show_boot_sequence();
  if (!pomodoro.begin()) {
    Serial.println("KAIROS_WARN pomodoro persistence unavailable");
  }
  pomodoro.set_listen_callback(request_listen);
  app.begin();
  gateway.begin();
  start_wifi_if_needed();
}

void loop() {
  const std::uint32_t now_ms = millis();
  poll_wifi_portal(now_ms);
  if (!portal_started) {
    gateway.loop(now_ms);
  }
  poll_touch(now_ms);
  app.tick(now_ms);
  delay(5);
}
